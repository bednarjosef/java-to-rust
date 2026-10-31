// A world of chunks, each with sections that hold an array of block states, as the Minecraft
// server keeps them: every tick reads a few fields of every section of every chunk, while chunks
// are unloaded and loaded again and short-lived objects of many sizes die between them. Loading a
// section makes temporary objects of the sizes it keeps, as decoding does, so the objects that
// stay end up between holes a few slots long: after a collection, a non-moving heap hands out its
// space in runs that short. What it measures is the cost of allocating from those holes, and how
// close together a chunk's objects end up in the heap: the reads are a pointer chase through
// objects made together. The time of the reads alone goes to standard error as `kernel-ms`.
public class ChunkSections {
    static final int CHUNKS = 3000;
    static final int SECTIONS = 16;
    static final int TEMPORARIES = 4;
    static Object sink;

    static class Section {
        int nonEmpty;
        int random;
        long[] states;
        Object palette;

        Section(int seed) {
            for (int t = 0; t < TEMPORARIES; t++) {
                sink = new Garbage(seed, sink);
                sink = new long[32];
                sink = new int[] {t, t, t};
            }
            nonEmpty = seed & 255;
            random = seed % 7;
            states = new long[32];
            for (int i = 0; i < states.length; i++) {
                states[i] = seed * 31L + i;
            }
            palette = new int[] {seed, seed + 1, seed + 2};
        }
    }

    static class Chunk {
        final int x;
        final Section[] sections = new Section[SECTIONS];
        long inhabited;

        Chunk(int x) {
            this.x = x;
            for (int i = 0; i < SECTIONS; i++) {
                sections[i] = new Section(x * SECTIONS + i);
            }
        }
    }

    static class Garbage {
        long a;
        Object b;

        Garbage(long a, Object b) {
            this.a = a;
            this.b = b;
        }
    }

    public static void main(String[] args) {
        Chunk[] world = new Chunk[CHUNKS];
        for (int i = 0; i < CHUNKS; i++) {
            world[i] = new Chunk(i);
        }
        long sum = 0;
        long seed = 12345;
        Object keep = null;
        long reading = 0;
        for (int tick = 0; tick < 400; tick++) {
            // Unload and load a few chunks, in a scattered order.
            for (int k = 0; k < 40; k++) {
                seed = seed * 6364136223846793005L + 1442695040888963407L;
                int i = (int) ((seed >>> 33) % CHUNKS);
                world[i] = new Chunk(i + tick);
            }
            // Short-lived objects of several sizes, some of which live a little longer.
            for (int k = 0; k < 20000; k++) {
                Object o = switch (k % 4) {
                    case 0 -> new Garbage(k, keep);
                    case 1 -> new long[(k & 7) + 1];
                    case 2 -> new int[(k & 15) + 3];
                    default -> new Section(k);
                };
                if ((k & 1023) == 0) {
                    keep = o;
                }
            }
            // The tick: a few fields of every section of every chunk.
            long start = System.nanoTime();
            for (Chunk chunk : world) {
                for (Section section : chunk.sections) {
                    if (section.nonEmpty > 0) {
                        sum += section.random + section.states[tick & 31];
                    }
                }
                chunk.inhabited++;
            }
            reading += System.nanoTime() - start;
        }
        System.err.println("kernel-ms " + reading / 1_000_000);
        System.out.println(sum + " " + (keep != null));
    }
}
