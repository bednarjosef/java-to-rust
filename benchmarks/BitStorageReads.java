// Values read from containers of packed bits through a palette, as the Minecraft server reads its
// chunks' block states (`PalettedContainer.get`): the container's data is a volatile record of a
// storage and a palette, the storage (`SimpleBitStorage.get`) checks the index with
// `Validate.inclusiveBetween` and divides it by the values a long holds with a multiply by a magic
// constant, and the palette maps the value to an object. The containers are built on one thread
// and read on another, as the server's are.
import java.util.ArrayList;
import java.util.List;

public class BitStorageReads {
    static final class Validate {
        static void inclusiveBetween(long start, long end, long value) {
            if (value < start || value > end) {
                throw new IllegalArgumentException(String.format("The value %s is not in the specified inclusive range of %s to %s", value, start, end));
            }
        }
    }

    interface BitStorage {
        int get(int index);

        void set(int index, int value);
    }

    static final class SimpleBitStorage implements BitStorage {
        private static final int[] MAGIC = {
            -1, -1, 0, Integer.MIN_VALUE, 0, 0, 1431655765, 1431655765, 0, Integer.MIN_VALUE, 0, 1, 858993459, 858993459, 0, 715827882,
            715827882, 0, 613566756, 613566756, 0, Integer.MIN_VALUE, 0, 2, 477218588, 477218588, 0, 429496729, 429496729, 0, 390451572,
            390451572, 0, 357913941, 357913941, 0, 330382099, 330382099, 0, 306783378, 306783378, 0, 286331153, 286331153, 0,
            Integer.MIN_VALUE, 0, 3, 252645135, 252645135, 0, 238609294, 238609294, 0, 226050910, 226050910, 0, 214748364, 214748364, 0,
            204522252, 204522252, 0, 195225786, 195225786, 0, 186737708, 186737708, 0, 178956970, 178956970, 0, 171798691, 171798691, 0,
            165191049, 165191049, 0, 159072862, 159072862, 0, 153391689, 153391689, 0, 148102320, 148102320, 0, 143165576, 143165576, 0,
            138547332, 138547332, 0, Integer.MIN_VALUE, 0, 4, 130150524, 130150524, 0, 126322567, 126322567, 0, 122713351, 122713351, 0,
            119304647, 119304647, 0, 116080197, 116080197, 0, 113025455, 113025455, 0, 110127366, 110127366, 0, 107374182, 107374182, 0,
            104755299, 104755299, 0, 102261126, 102261126, 0, 99882960, 99882960, 0, 97612893, 97612893, 0, 95443717, 95443717, 0,
            93368854, 93368854, 0, 91382282, 91382282, 0, 89478485, 89478485, 0, 87652393, 87652393, 0, 85899345, 85899345, 0, 84215045,
            84215045, 0, 82595524, 82595524, 0, 81037118, 81037118, 0, 79536431, 79536431, 0, 78090314, 78090314, 0, 76695844, 76695844,
            0, 75350303, 75350303, 0, 74051160, 74051160, 0, 72796055, 72796055, 0, 71582788, 71582788, 0, 70409299, 70409299, 0,
            69273666, 69273666, 0, 68174084, 68174084, 0, Integer.MIN_VALUE, 0, 5
        };
        private final long[] data;
        private final int bits;
        private final long mask;
        private final int size;
        private final int valuesPerLong;
        private final int divideMul;
        private final int divideAdd;
        private final int divideShift;

        SimpleBitStorage(int bits, int size) {
            this.size = size;
            this.bits = bits;
            this.mask = (1L << bits) - 1L;
            this.valuesPerLong = (char) (64 / bits);
            int magicIndex = 3 * (this.valuesPerLong - 1);
            this.divideMul = MAGIC[magicIndex + 0];
            this.divideAdd = MAGIC[magicIndex + 1];
            this.divideShift = MAGIC[magicIndex + 2];
            this.data = new long[(size + this.valuesPerLong - 1) / this.valuesPerLong];
        }

        private int cellIndex(int bitIndex) {
            long mul = Integer.toUnsignedLong(this.divideMul);
            long add = Integer.toUnsignedLong(this.divideAdd);
            return (int) (bitIndex * mul + add >> 32 >> this.divideShift);
        }

        public int get(int index) {
            Validate.inclusiveBetween(0L, this.size - 1, index);
            int cellIndex = this.cellIndex(index);
            long cellValue = this.data[cellIndex];
            int bitIndex = (index - cellIndex * this.valuesPerLong) * this.bits;
            return (int) (cellValue >> bitIndex & this.mask);
        }

        public void set(int index, int value) {
            Validate.inclusiveBetween(0L, this.size - 1, index);
            Validate.inclusiveBetween(0L, this.mask, value);
            int cellIndex = this.cellIndex(index);
            long cellValue = this.data[cellIndex];
            int bitIndex = (index - cellIndex * this.valuesPerLong) * this.bits;
            this.data[cellIndex] = cellValue & ~(this.mask << bitIndex) | (value & this.mask) << bitIndex;
        }
    }

    static final class ZeroBitStorage implements BitStorage {
        private final int size;

        ZeroBitStorage(int size) {
            this.size = size;
        }

        public int get(int index) {
            Validate.inclusiveBetween(0L, this.size - 1, index);
            return 0;
        }

        public void set(int index, int value) {
            Validate.inclusiveBetween(0L, this.size - 1, index);
            Validate.inclusiveBetween(0L, 0L, value);
        }
    }

    static final class State {
        final int id;

        State(int id) {
            this.id = id;
        }
    }

    interface Palette {
        State valueFor(int id);
    }

    static final class LinearPalette implements Palette {
        private final State[] values;

        LinearPalette(State[] values) {
            this.values = values;
        }

        public State valueFor(int id) {
            if (id >= 0 && id < this.values.length) {
                State value = this.values[id];
                if (value != null) {
                    return value;
                }
            }
            throw new IllegalStateException("Missing palette entry for index " + id);
        }
    }

    static final class SingleValuePalette implements Palette {
        private final State value;

        SingleValuePalette(State value) {
            this.value = value;
        }

        public State valueFor(int id) {
            if (id != 0) {
                throw new IllegalStateException("Missing palette entry for index " + id);
            }
            return this.value;
        }
    }

    record Data(BitStorage storage, Palette palette) {}

    static final class Container {
        private volatile Data data;

        Container(Data data) {
            this.data = data;
        }

        State get(int index) {
            Data data = this.data;
            return data.palette.valueFor(data.storage.get(index));
        }
    }

    static long run(List<Container> containers, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (Container c : containers) {
                for (int i = 0; i < 4096; i++) {
                    sum += c.get(i).id;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 400;
        List<Container> containers = new ArrayList<>();
        long seed = 42;
        for (int k = 0; k < 24; k++) {
            int bits = 4 + k % 3;
            State[] states = new State[1 << bits];
            for (int s = 0; s < states.length; s++) {
                states[s] = new State(k * 100 + s);
            }
            SimpleBitStorage storage = new SimpleBitStorage(bits, 4096);
            for (int i = 0; i < 4096; i++) {
                seed = seed * 6364136223846793005L + 1442695040888963407L;
                storage.set(i, (int) (seed >>> 33) & ((1 << bits) - 1));
            }
            containers.add(new Container(new Data(storage, new LinearPalette(states))));
            if (k % 8 == 7) {
                containers.add(new Container(new Data(new ZeroBitStorage(4096), new SingleValuePalette(new State(k)))));
            }
        }
        long[] result = new long[2];
        Thread reader = new Thread(() -> {
            result[0] = run(containers, 20);
            long start = System.nanoTime();
            result[1] = run(containers, rounds);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        reader.start();
        reader.join();
        System.out.println("bit storage reads " + result[0] + " " + result[1]);
    }
}
