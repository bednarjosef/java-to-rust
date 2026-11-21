// The Minecraft server's random block ticks (`ServerLevel.tickChunk`), its hottest loop, with the
// same chain of objects: the level's chunks give their sections through a getter; for each
// section that ticks, a few random positions each read a block state through the section's
// container (a final field set from a constructor's argument), the container's strategy and its
// volatile data (a record of a storage and a palette, which a resize replaces), the storage's
// packed longs and the palette's array, cast back from the palette's type variable; then the
// state's fluid state, which the state's cache sets after construction, and the fluid's type. A
// profiler pushes and pops around each tick, and the level's own random number is a field it
// updates in place. Java tests none of these references for null explicitly, and reads a field
// once where nothing between could change it. It runs on a worker thread, as the server's ticks
// do, so the objects' fields are ones that threads may share.
public class RandomTickReads {
    static final int CHUNKS = 96;
    static final int SECTIONS = 24;

    abstract static class Fluid {
        abstract boolean isRandomlyTicking();

        abstract int amount();
    }

    static final class EmptyFluid extends Fluid {
        boolean isRandomlyTicking() {
            return false;
        }

        int amount() {
            return 0;
        }
    }

    static final class FlowingFluid extends Fluid {
        final int level;

        FlowingFluid(int level) {
            this.level = level;
        }

        boolean isRandomlyTicking() {
            return level == 7;
        }

        int amount() {
            return level;
        }
    }

    static final class FluidState {
        private final Fluid owner;

        FluidState(Fluid owner) {
            this.owner = owner;
        }

        Fluid getType() {
            return owner;
        }

        boolean isRandomlyTicking() {
            return getType().isRandomlyTicking();
        }
    }

    static final class BlockState {
        final int id;
        private final Fluid fluid;
        private FluidState fluidState;
        private boolean randomlyTicking;
        int ticks;

        BlockState(int id, Fluid fluid) {
            this.id = id;
            this.fluid = fluid;
        }

        // As `BlockStateBase.initCache`, which the registry runs once every state exists.
        void initCache(FluidState[] fluidStates) {
            this.fluidState = fluidStates[id % fluidStates.length];
            this.randomlyTicking = id % 13 == 0;
        }

        FluidState getFluidState() {
            return fluidState;
        }

        boolean isRandomlyTicking() {
            return randomlyTicking;
        }

        void randomTick(Level level, BlockPos pos) {
            ticks += pos.getX() + level.ticked;
        }
    }

    static class Vec3i {
        private final int x;
        private final int y;
        private final int z;

        Vec3i(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        int getX() {
            return x;
        }

        int getY() {
            return y;
        }

        int getZ() {
            return z;
        }
    }

    static class BlockPos extends Vec3i {
        BlockPos(int x, int y, int z) {
            super(x, y, z);
        }
    }

    static final class MutableBlockPos extends BlockPos {
        MutableBlockPos(int x, int y, int z) {
            super(x, y, z);
        }
    }

    static void inclusiveBetween(long start, long end, long value) {
        if (value < start || value > end) {
            throw new IllegalArgumentException("The value " + value + " is not in the range of " + start + " to " + end);
        }
    }

    interface BitStorage {
        int get(int index);

        void set(int index, int value);
    }

    static final class SimpleBitStorage implements BitStorage {
        private final long[] data;
        private final int bits;
        private final long mask;
        private final int size;
        private final int valuesPerLong;

        SimpleBitStorage(int bits, int size) {
            this(bits, size, null);
        }

        // As the server's, which checks its arguments first and takes the longs of a saved chunk
        // or makes new ones.
        SimpleBitStorage(int bits, int size, long[] data) {
            inclusiveBetween(1L, 32L, bits);
            this.size = size;
            this.bits = bits;
            this.mask = (1L << bits) - 1L;
            this.valuesPerLong = 64 / bits;
            int requiredLength = (size + valuesPerLong - 1) / valuesPerLong;
            if (data != null) {
                if (data.length != requiredLength) {
                    throw new IllegalStateException("Invalid length given for storage, got: " + data.length + " but expected: " + requiredLength);
                }
                this.data = data;
            } else {
                this.data = new long[requiredLength];
            }
        }

        public int get(int index) {
            inclusiveBetween(0L, size - 1, index);
            int cellIndex = index / valuesPerLong;
            long cellValue = data[cellIndex];
            int bitIndex = (index - cellIndex * valuesPerLong) * bits;
            return (int) (cellValue >> bitIndex & mask);
        }

        public void set(int index, int value) {
            inclusiveBetween(0L, size - 1, index);
            int cellIndex = index / valuesPerLong;
            int bitIndex = (index - cellIndex * valuesPerLong) * bits;
            data[cellIndex] = data[cellIndex] & ~(mask << bitIndex) | (value & mask) << bitIndex;
        }
    }

    static final class ZeroBitStorage implements BitStorage {
        private final int size;

        ZeroBitStorage(int size) {
            this.size = size;
        }

        public int get(int index) {
            inclusiveBetween(0L, size - 1, index);
            return 0;
        }

        public void set(int index, int value) {
            inclusiveBetween(0L, size - 1, index);
            inclusiveBetween(0L, 0L, value);
        }
    }

    interface PaletteResize<T> {
        int onResize(int bits, T lastAddedValue);
    }

    interface Palette<T> {
        T valueFor(int index);

        int idFor(T value, PaletteResize<T> resizer);

        int getSize();
    }

    static final class LinearPalette<T> implements Palette<T> {
        private final T[] values;
        private final int bits;
        private int size;

        @SuppressWarnings("unchecked")
        LinearPalette(int bits) {
            this.values = (T[]) new Object[1 << bits];
            this.bits = bits;
        }

        public T valueFor(int index) {
            if (index >= 0 && index < size) {
                return values[index];
            }
            throw new IllegalStateException("Missing palette entry for index " + index);
        }

        public int idFor(T value, PaletteResize<T> resizer) {
            for (int i = 0; i < size; i++) {
                if (values[i] == value) {
                    return i;
                }
            }
            if (size < values.length) {
                values[size] = value;
                return size++;
            }
            return resizer.onResize(bits + 1, value);
        }

        public int getSize() {
            return size;
        }
    }

    static final class SingleValuePalette<T> implements Palette<T> {
        private T value;

        public T valueFor(int index) {
            if (value == null || index != 0) {
                throw new IllegalStateException("Missing palette entry for index " + index);
            }
            return value;
        }

        public int idFor(T value, PaletteResize<T> resizer) {
            if (this.value != null && this.value != value) {
                return resizer.onResize(1, value);
            }
            this.value = value;
            return 0;
        }

        public int getSize() {
            return 1;
        }
    }

    static final class Strategy {
        private final int bitsPerAxis;

        Strategy(int bitsPerAxis) {
            this.bitsPerAxis = bitsPerAxis;
        }

        int getIndex(int x, int y, int z) {
            return (y << bitsPerAxis | z) << bitsPerAxis | x;
        }

        int size() {
            return 1 << bitsPerAxis * 3;
        }
    }

    record Data<T>(int bits, BitStorage storage, Palette<T> palette) {}

    // `PalettedContainer`: its constructor passes `this` to the palette as the resizer, and a
    // resize replaces the volatile data with data that may be the old one.
    static final class Container<T> implements PaletteResize<T> {
        private volatile Data<T> data;
        private final Strategy strategy;

        Container(T initialValue, Strategy strategy) {
            this.strategy = strategy;
            this.data = createOrReuseData(null, 0);
            this.data.palette().idFor(initialValue, this);
        }

        private Data<T> createOrReuseData(Data<T> oldData, int bits) {
            if (oldData != null && oldData.bits() == bits) {
                return oldData;
            }
            int size = strategy.size();
            if (bits == 0) {
                return new Data<>(0, new ZeroBitStorage(size), new SingleValuePalette<>());
            }
            int storageBits = Math.max(4, bits);
            return new Data<>(bits, new SimpleBitStorage(storageBits, size), new LinearPalette<>(storageBits));
        }

        public int onResize(int bits, T lastAddedValue) {
            Data<T> oldData = this.data;
            Data<T> newData = createOrReuseData(oldData, bits);
            if (newData != oldData) {
                int size = strategy.size();
                for (int i = 0; i < size; i++) {
                    T value = oldData.palette().valueFor(oldData.storage().get(i));
                    newData.storage().set(i, newData.palette().idFor(value, this));
                }
            }
            this.data = newData;
            return newData.palette().idFor(lastAddedValue, this);
        }

        T get(int x, int y, int z) {
            return get(strategy.getIndex(x, y, z));
        }

        T get(int index) {
            Data<T> data = this.data;
            return data.palette().valueFor(data.storage().get(index));
        }

        void set(int x, int y, int z, T value) {
            int index = strategy.getIndex(x, y, z);
            int id = this.data.palette().idFor(value, this);
            this.data.storage().set(index, id);
        }
    }

    static final class Section {
        private short nonEmptyBlockCount;
        private short tickingBlockCount;
        private short tickingFluidCount;
        private final Container<BlockState> states;

        Section(Container<BlockState> states, int ticking) {
            this.states = states;
            this.nonEmptyBlockCount = 4096;
            this.tickingBlockCount = (short) ticking;
            this.tickingFluidCount = (short) (ticking & 1);
        }

        boolean isRandomlyTicking() {
            return tickingBlockCount > 0 || tickingFluidCount > 0;
        }

        boolean hasOnlyAir() {
            return nonEmptyBlockCount == 0;
        }

        BlockState getBlockState(int x, int y, int z) {
            return states.get(x, y, z);
        }
    }

    interface HeightAccessor {
        int getMinY();

        default int getMinSectionY() {
            return getMinY() >> 4;
        }

        default int getSectionYFromSectionIndex(int index) {
            return index + getMinSectionY();
        }
    }

    static final class Chunk implements HeightAccessor {
        private final Section[] sections;
        private final int x;
        private final int z;
        private final Level level;

        Chunk(Level level, int x, int z, Section[] sections) {
            this.level = level;
            this.x = x;
            this.z = z;
            this.sections = sections;
        }

        Section[] getSections() {
            return sections;
        }

        public int getMinY() {
            return level.minY;
        }

        int getMinBlockX() {
            return x << 4;
        }

        int getMinBlockZ() {
            return z << 4;
        }
    }

    interface Profiler {
        void push(String name);

        void pop();
    }

    static final class InactiveProfiler implements Profiler {
        public void push(String name) {}

        public void pop() {}
    }

    static final class CountingProfiler implements Profiler {
        int depth;
        int pushes;

        public void push(String name) {
            depth++;
            pushes++;
        }

        public void pop() {
            depth--;
        }
    }

    static final class Level {
        final int minY = -64;
        int randValue = 1;
        int ticked;
        Profiler profiler;
        Chunk[] chunks;

        BlockPos getBlockRandomPos(int x, int y, int z, int mask) {
            randValue = randValue * 3 + 1013904223;
            int val = randValue >> 2;
            return new BlockPos(x + (val & mask), y + (val >> 16 & mask), z + (val >> 8 & mask));
        }

        void tickChunk(Chunk chunk, int tickSpeed) {
            int minX = chunk.getMinBlockX();
            int minZ = chunk.getMinBlockZ();
            Profiler profiler = this.profiler;
            profiler.push("tickBlocks");
            Section[] sections = chunk.getSections();
            for (int sectionIndex = 0; sectionIndex < sections.length; sectionIndex++) {
                Section section = sections[sectionIndex];
                if (section.isRandomlyTicking()) {
                    int sectionY = chunk.getSectionYFromSectionIndex(sectionIndex);
                    int minYInSection = sectionY << 4;
                    for (int i = 0; i < tickSpeed; i++) {
                        BlockPos pos = getBlockRandomPos(minX, minYInSection, minZ, 15);
                        profiler.push("randomTick");
                        BlockState blockState = section.getBlockState(pos.getX() - minX, pos.getY() - minYInSection, pos.getZ() - minZ);
                        if (blockState.isRandomlyTicking()) {
                            blockState.randomTick(this, pos);
                        }
                        FluidState fluidState = blockState.getFluidState();
                        if (fluidState.isRandomlyTicking()) {
                            ticked++;
                        }
                        profiler.pop();
                    }
                }
            }
            profiler.pop();
        }
    }

    // A container of the section's states filled from `seed`, or null for a section that the
    // save leaves out, as a codec's result may be.
    static Container<BlockState> container(BlockState[] states, Strategy strategy, int k, long seed) {
        if (k < 0) {
            return null;
        }
        int kinds = 2 + k % 14;
        Container<BlockState> container = new Container<>(states[0], strategy);
        for (int y = 0; y < 16; y++) {
            for (int z = 0; z < 16; z++) {
                for (int x = 0; x < 16; x++) {
                    seed = seed * 6364136223846793005L + 1442695040888963407L;
                    int kind = (int) (seed >>> 33) % kinds;
                    container.set(x, y, z, states[(k * 7 + kind) % states.length]);
                }
            }
        }
        return container;
    }

    static long tick(Level level, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (Chunk chunk : level.chunks) {
                level.tickChunk(chunk, 3);
            }
            sum += level.ticked + level.randValue;
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 1500;
        Fluid[] fluids = new Fluid[3];
        fluids[0] = new EmptyFluid();
        fluids[1] = new FlowingFluid(3);
        fluids[2] = new FlowingFluid(7);
        FluidState[] fluidStates = new FluidState[fluids.length];
        for (int f = 0; f < fluids.length; f++) {
            fluidStates[f] = new FluidState(fluids[f]);
        }
        BlockState[] states = new BlockState[200];
        for (int s = 0; s < states.length; s++) {
            states[s] = new BlockState(s, fluids[s % 3]);
        }
        for (BlockState state : states) {
            state.initCache(fluidStates);
        }
        Level level = new Level();
        states[0].randomTick(level, new MutableBlockPos(1, 2, 3));
        level.profiler = args.length > 1 ? new CountingProfiler() : new InactiveProfiler();
        Strategy strategy = new Strategy(4);
        level.chunks = new Chunk[CHUNKS];
        long seed = 42;
        for (int c = 0; c < CHUNKS; c++) {
            Section[] sections = new Section[SECTIONS];
            for (int s = 0; s < SECTIONS; s++) {
                seed = seed * 6364136223846793005L + 1442695040888963407L;
                sections[s] = new Section(container(states, strategy, c + s, seed), (c + s) % 5);
            }
            level.chunks[c] = new Chunk(level, c % 12, c / 12, sections);
        }
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            result[0] = tick(level, 20);
            long start = System.nanoTime();
            result[1] = tick(level, rounds);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        long stateTicks = 0;
        for (BlockState state : states) {
            stateTicks += state.ticks;
        }
        System.out.println("random tick reads " + result[0] + " " + result[1] + " " + level.ticked + " " + stateTicks);
    }
}
