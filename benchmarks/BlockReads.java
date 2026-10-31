// The Minecraft server's random block ticks: every tick reads a few blocks at random positions of
// each loaded chunk section, each read through the section's bit-packed storage (a long[] of
// palette indices) and its palette (an array of block states). The sections take about 60 MB,
// so most reads miss the cache, and a read costs about as many waits for memory as it has loads
// that depend on each other: the section, its storage, the storage's array, the palette, the
// state. It runs on a worker thread, as the server's ticks do, so the arrays are ones that
// threads share.
public class BlockReads {
    static final class BlockState {
        final int id;
        final boolean ticking;

        BlockState(int id, boolean ticking) {
            this.id = id;
            this.ticking = ticking;
        }
    }

    static final class BitStorage {
        final long[] data;
        final int bits;
        final long mask;
        final int valuesPerLong;
        final int size;

        BitStorage(int bits, int size) {
            this.bits = bits;
            this.size = size;
            this.mask = (1L << bits) - 1L;
            this.valuesPerLong = 64 / bits;
            this.data = new long[(size + valuesPerLong - 1) / valuesPerLong];
        }

        int get(int index) {
            if (index < 0 || index >= size) {
                throw new IndexOutOfBoundsException("index " + index + " of " + size);
            }
            int cell = index / valuesPerLong;
            long value = data[cell];
            int bit = (index - cell * valuesPerLong) * bits;
            return (int) (value >> bit & mask);
        }

        void set(int index, int value) {
            int cell = index / valuesPerLong;
            int bit = (index - cell * valuesPerLong) * bits;
            data[cell] = data[cell] & ~(mask << bit) | ((long) value & mask) << bit;
        }
    }

    static final class Section {
        final BitStorage storage;
        final BlockState[] palette;
        short tickingBlocks;

        Section(BitStorage storage, BlockState[] palette) {
            this.storage = storage;
            this.palette = palette;
        }

        BlockState get(int x, int y, int z) {
            return palette[storage.get(y << 8 | z << 4 | x)];
        }
    }

    static final class Chunk {
        final Section[] sections;

        Chunk(Section[] sections) {
            this.sections = sections;
        }
    }

    static final class Xorshift {
        int seed = 12345;

        int next() {
            seed ^= seed << 13;
            seed ^= seed >>> 17;
            seed ^= seed << 5;
            return seed;
        }
    }

    static Chunk[] chunks(int count, BlockState[] states, Xorshift random) {
        Chunk[] chunks = new Chunk[count];
        for (int c = 0; c < count; c++) {
            Section[] sections = new Section[24];
            for (int s = 0; s < sections.length; s++) {
                BlockState[] palette = new BlockState[16];
                for (int p = 0; p < palette.length; p++) {
                    palette[p] = states[(random.next() & 0x7fffffff) % states.length];
                }
                BitStorage storage = new BitStorage(4, 4096);
                Section section = new Section(storage, palette);
                for (int i = 0; i < 4096; i++) {
                    int p = random.next() & 15;
                    storage.set(i, p);
                    if (palette[p].ticking) {
                        section.tickingBlocks++;
                    }
                }
                sections[s] = section;
            }
            chunks[c] = new Chunk(sections);
        }
        return chunks;
    }

    // One tick: three random reads in each section that has ticking blocks.
    static long tick(Chunk[] chunks, Xorshift random) {
        long sum = 0;
        for (Chunk chunk : chunks) {
            Section[] sections = chunk.sections;
            for (int s = 0; s < sections.length; s++) {
                Section section = sections[s];
                if (section.tickingBlocks > 0) {
                    for (int i = 0; i < 3; i++) {
                        int r = random.next();
                        BlockState state = section.get(r & 15, r >>> 8 & 15, r >>> 4 & 15);
                        if (state.ticking) {
                            sum += state.id;
                        }
                    }
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        BlockState[] states = new BlockState[64];
        for (int i = 0; i < states.length; i++) {
            states[i] = new BlockState(i, i % 3 == 0);
        }
        Xorshift random = new Xorshift();
        Chunk[] chunks = chunks(1024, states, random);
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            for (int t = 0; t < 20; t++) {
                result[0] += tick(chunks, random);
            }
            long start = System.nanoTime();
            for (int t = 0; t < 400; t++) {
                result[1] += tick(chunks, random);
            }
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println(result[0] + " " + result[1]);
    }
}
