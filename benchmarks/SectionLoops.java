// The loops of the Minecraft server's chunk ticks over arrays it reaches through getters and
// fields: `ServerLevel.tickChunk` takes `chunk.getSections()`, an array that may be null as far as
// the translator can tell, and walks it with `for (i = 0; i < sections.length; i++)`, calling
// methods on each section; a section's own counts are a `final` array field walked by its
// methods. Java's JIT drops the bounds checks of both loops. It runs on a worker thread, as the
// server's ticks do, so the arrays are ones that threads share.
public class SectionLoops {
    static final class Section {
        private final short[] counts;
        private int ticks;

        Section(int seed) {
            counts = new short[16];
            for (int i = 0; i < counts.length; i++) {
                counts[i] = (short) ((seed * 31 + i * 7) % 5);
            }
        }

        boolean isRandomlyTicking() {
            return counts[0] > 0 || counts[1] > 0;
        }

        int weight() {
            int w = 0;
            for (int i = 0; i < counts.length; i++) {
                w += counts[i] * (i + 1);
            }
            return w;
        }

        void tick(int r) {
            ticks++;
            counts[r & 15] = (short) ((counts[r & 15] + 1) % 5);
        }
    }

    static final class Chunk {
        private final Section[] sections;

        Chunk(int seed) {
            sections = new Section[24];
            for (int i = 0; i < sections.length; i++) {
                sections[i] = new Section(seed + i);
            }
        }

        // A chunk that is not loaded has no sections.
        Chunk() {
            sections = null;
        }

        Section[] getSections() {
            return sections;
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

    static long tickChunk(Chunk chunk, int tickSpeed, Xorshift random) {
        long sum = 0;
        if (tickSpeed > 0) {
            Section[] sections = chunk.getSections();
            for (int sectionIndex = 0; sectionIndex < sections.length; sectionIndex++) {
                Section section = sections[sectionIndex];
                if (section.isRandomlyTicking()) {
                    for (int i = 0; i < tickSpeed; i++) {
                        section.tick(random.next());
                    }
                    sum += section.weight();
                }
            }
        }
        return sum;
    }

    static long tick(Chunk[] chunks, Xorshift random) {
        long sum = 0;
        for (Chunk chunk : chunks) {
            sum += tickChunk(chunk, 3, random);
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        Chunk[] chunks = new Chunk[512];
        for (int c = 0; c < chunks.length; c++) {
            chunks[c] = new Chunk(c);
        }
        if (args.length > 99) {
            chunks[0] = new Chunk();
        }
        Xorshift random = new Xorshift();
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            for (int t = 0; t < 20; t++) {
                result[0] += tick(chunks, random);
            }
            long start = System.nanoTime();
            for (int t = 0; t < 1000; t++) {
                result[1] += tick(chunks, random);
            }
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println(result[0] + " " + result[1]);
    }
}
