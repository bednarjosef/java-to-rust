// Calls through interfaces that a few classes implement, as a game's block palettes and the
// accessors of a level's heights are: each call picks one of three or four methods by the
// object's class, and each method does little. The translation makes a call through an
// interface a call of a function that matches the class and calls the method; when a program is
// split into several crates, that function can lie in another crate than its callers.
public class FewImplementations {
    interface Palette {
        int valueFor(int id);

        int size();
    }

    static final class Single implements Palette {
        final int value;

        Single(int value) { this.value = value; }

        public int valueFor(int id) { return value; }

        public int size() { return 1; }
    }

    static final class Linear implements Palette {
        final int[] values;

        Linear(int[] values) { this.values = values; }

        public int valueFor(int id) { return values[id & 15]; }

        public int size() { return values.length; }
    }

    static final class Hashed implements Palette {
        final int[] values;

        Hashed(int[] values) { this.values = values; }

        public int valueFor(int id) { return values[(id * 31) & 255]; }

        public int size() { return values.length; }
    }

    static final class Global implements Palette {
        public int valueFor(int id) { return id; }

        public int size() { return 1 << 16; }
    }

    interface Heights {
        int minY();

        int height();
    }

    static final class Level implements Heights {
        final int minY;
        final int height;

        Level(int minY, int height) { this.minY = minY; this.height = height; }

        public int minY() { return minY; }

        public int height() { return height; }
    }

    static final class Region implements Heights {
        final Level level;

        Region(Level level) { this.level = level; }

        public int minY() { return level.minY; }

        public int height() { return level.height; }
    }

    static final class Proto implements Heights {
        public int minY() { return -64; }

        public int height() { return 384; }
    }

    public static void main(String[] args) {
        int[] small = new int[16];
        int[] large = new int[256];
        for (int i = 0; i < large.length; i++) {
            large[i] = i * 7 + 3;
            if (i < small.length) small[i] = i * 3 + 1;
        }
        Palette[] palettes = new Palette[1024];
        Heights[] heights = new Heights[1024];
        Level level = new Level(-64, 384);
        long seed = 42;
        for (int i = 0; i < palettes.length; i++) {
            seed = seed * 6364136223846793005L + 1442695040888963407L;
            int kind = (int) (seed >>> 60) & 3;
            palettes[i] = kind == 0 ? new Single(i) : kind == 1 ? new Linear(small) : kind == 2 ? new Hashed(large) : new Global();
            int h = (int) (seed >>> 40) % 3;
            heights[i] = h == 0 ? level : h == 1 ? new Region(level) : new Proto();
        }
        long start = System.nanoTime();
        long sum = 0;
        for (int round = 0; round < 40000; round++) {
            for (int i = 0; i < palettes.length; i++) {
                Palette p = palettes[i];
                Heights h = heights[(i + round) & 1023];
                sum += p.valueFor(i + round) + p.size() + h.minY() + h.height();
            }
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        System.out.println(sum);
    }
}
