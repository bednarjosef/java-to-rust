// Reads through fields that constructors set after other work, as the Minecraft server's chunk
// sections do: a section's container reads its strategy to index a block and its data to find it
// (`PalettedContainer.get`), and the constructors check their arguments, compute, and register the
// new object before or after they set those fields. Each read of such a field once tested it for
// null; the fields are never null once built.
public class FieldsSetLate {
    static final Object[] REGISTERED = new Object[64];
    static int registered = 0;

    static void check(boolean ok, String what) {
        if (!ok) {
            throw new IllegalArgumentException(what);
        }
    }

    static final class Strategy {
        final int bits;
        final int size;

        Strategy(int bits) {
            check(bits > 0 && bits < 8, "bits");
            this.bits = bits;
            this.size = 1 << (3 * bits);
        }

        int index(int x, int y, int z) {
            return (y << bits | z) << bits | x;
        }
    }

    static final class Data {
        final int[] values;
        final Object[] palette;

        Data(int size, Object[] palette) {
            check(size > 0, "size");
            int[] values = new int[size];
            for (int i = 0; i < size; i++) {
                values[i] = (i * 31 + i / 7) % palette.length;
            }
            this.values = values;
            this.palette = palette;
        }
    }

    static final class Container {
        final Strategy strategy;
        Data data;

        Container(Strategy strategy, Object[] palette) {
            this.strategy = strategy;
            this.data = new Data(strategy.size, palette);
            REGISTERED[registered++] = this;
        }

        Object get(int x, int y, int z) {
            Data d = this.data;
            return d.palette[d.values[strategy.index(x, y, z)]];
        }
    }

    static final class Section {
        final Container states;
        final String name;
        int ticking;

        Section(int y, Strategy strategy, Object[] palette) {
            check(y >= 0, "y");
            this.name = "section " + y;
            this.states = new Container(strategy, palette);
            REGISTERED[registered++] = this;
        }
    }

    public static void main(String[] args) {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 40000;
        Strategy strategy = new Strategy(4);
        Object[] palette = new Object[16];
        for (int i = 0; i < palette.length; i++) {
            palette[i] = i % 5 == 0 ? "stone" : "air";
        }
        Section[] sections = new Section[24];
        for (int y = 0; y < sections.length; y++) {
            sections[y] = new Section(y, strategy, palette);
        }
        long stone = 0;
        long start = System.nanoTime();
        for (int r = 0; r < rounds; r++) {
            for (Section s : sections) {
                for (int i = 0; i < 64; i++) {
                    int x = (i * 7 + r) & 15;
                    int y = (i * 3) & 15;
                    int z = (i * 5 + r) & 15;
                    if (s.states.get(x, y, z) == "stone") {
                        stone++;
                        s.ticking++;
                    }
                }
            }
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        int ticking = 0;
        for (Section s : sections) {
            ticking += s.ticking + s.name.length();
        }
        System.out.println("stone " + stone + " ticking " + ticking + " registered " + registered);
    }
}
