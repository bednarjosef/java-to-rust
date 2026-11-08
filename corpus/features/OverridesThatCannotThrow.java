// Override groups in which some members throw what a catch sees and the others cannot throw
// it: palettes looked up in a loop through an interface, where one palette's exception is
// unseen and the others' is caught, and shapes whose areas are summed through a superclass and
// called directly on a final class. One lookup and one area throw and are caught. The members
// that cannot throw return their values plainly, behind a function that gives the group its
// `Result`.
public class OverridesThatCannotThrow {
    static final class MissingEntry extends RuntimeException {
        MissingEntry(int index) { super("Missing entry for index " + index + "."); }
    }

    static final class State {
        final String name;
        final boolean ticking;
        State(String name, boolean ticking) { this.name = name; this.ticking = ticking; }
    }

    interface Palette { Object valueFor(int index); }

    static final class Linear implements Palette {
        final Object[] values;
        final int size;
        Linear(Object[] values, int size) { this.values = values; this.size = size; }
        public Object valueFor(int index) {
            if (index >= 0 && index < size) return values[index];
            throw new MissingEntry(index);
        }
    }

    static final class Single implements Palette {
        final Object value;
        Single(Object value) { this.value = value; }
        public Object valueFor(int index) {
            if (value == null) throw new IllegalStateException("Use of an uninitialized palette");
            return value;
        }
    }

    static final class Hashed implements Palette {
        final int[] keys = new int[8];
        final Object[] values = new Object[8];
        void put(int key, Object value) {
            int slot = key * 5 & 7;
            while (values[slot] != null && keys[slot] != key) slot = slot + 1 & 7;
            keys[slot] = key;
            values[slot] = value;
        }
        public Object valueFor(int index) {
            int slot = index * 5 & 7;
            while (values[slot] != null) {
                if (keys[slot] == index) return values[slot];
                slot = slot + 1 & 7;
            }
            throw new MissingEntry(index);
        }
    }

    static final class Global implements Palette {
        final Object[] all;
        Global(Object[] all) { this.all = all; }
        public Object valueFor(int index) {
            Object v = index >= 0 && index < all.length ? all[index] : null;
            if (v == null) throw new MissingEntry(index);
            return v;
        }
    }

    static final class Storage {
        final long[] data;
        final int bits;
        final long mask;
        final int perLong;
        Storage(int bits, int size) {
            this.bits = bits;
            this.mask = (1L << bits) - 1;
            this.perLong = 64 / bits;
            this.data = new long[(size + perLong - 1) / perLong];
        }
        int get(int index) {
            int cell = index / perLong;
            int shift = (index - cell * perLong) * bits;
            return (int) (data[cell] >> shift & mask);
        }
        void set(int index, int value) {
            int cell = index / perLong;
            int shift = (index - cell * perLong) * bits;
            data[cell] = data[cell] & ~(mask << shift) | ((long) value & mask) << shift;
        }
    }

    static final class Container {
        Palette palette;
        final Storage storage;
        Container(Palette palette, Storage storage) { this.palette = palette; this.storage = storage; }
        Object get(int index) { return palette.valueFor(storage.get(index)); }
    }

    static final class Section {
        final Container states;
        Section(Container states) { this.states = states; }
        State getBlockState(int x, int y, int z) { return (State) states.get((y << 8 | z << 4) | x); }
    }

    // Calls that cannot throw, through a dispatch over several classes.
    interface Weight { int weight(State s, int i); }
    static final class Flat implements Weight { public int weight(State s, int i) { return 1; } }
    static final class ByName implements Weight { public int weight(State s, int i) { return s.name.length() + (i & 1); } }

    // An override group in which one member throws what a catch sees: the others cannot throw.
    static abstract class Shape { abstract int area(int scale); }
    static final class Square extends Shape {
        final int side;
        Square(int side) { this.side = side; }
        int area(int scale) { return side * side * scale; }
    }
    static final class Circle extends Shape {
        final int radius;
        Circle(int radius) { this.radius = radius; }
        int area(int scale) {
            int sum = 0;
            for (int i = 0; i < radius; i++) sum += (2 * i + 1) * 3;
            return sum * scale;
        }
    }
    static final class Torn extends Shape {
        int area(int scale) {
            if (scale > 2) throw new MissingEntry(scale);
            return scale;
        }
    }

    static long areas(Shape[] shapes, Square[] squares, int rounds) {
        long total = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < shapes.length; i++) {
                total += shapes[i].area(r % 3 + 1);
                total += squares[i % squares.length].area(r & 7);
            }
        }
        return total;
    }

    static int seed = 12345;
    static int next() { seed = seed * 1103515245 + 12345; return seed >>> 8; }

    static long tick(Section[] sections, Weight[] weights, int rounds) {
        long ticked = 0;
        for (int r = 0; r < rounds; r++) {
            for (int s = 0; s < sections.length; s++) {
                Section section = sections[s];
                Weight w = weights[s % weights.length];
                for (int k = 0; k < 3; k++) {
                    int v = next();
                    State state = section.getBlockState(v & 15, v >> 4 & 15, v >> 8 & 15);
                    if (state.ticking) ticked += w.weight(state, k);
                }
            }
        }
        return ticked;
    }

    public static void main(String[] args) {
        State[] states = { new State("air", false), new State("grass", true), new State("stone", false), new State("leaves", true) };
        Object[] linearValues = new Object[16];
        for (int i = 0; i < 4; i++) linearValues[i] = states[i];
        Hashed hashed = new Hashed();
        for (int i = 0; i < 4; i++) hashed.put(i, states[3 - i]);
        Palette[] palettes = { new Linear(linearValues, 4), new Single(states[1]), hashed, new Global(states) };
        Section[] sections = new Section[8];
        for (int s = 0; s < sections.length; s++) {
            Storage storage = new Storage(4, 4096);
            for (int i = 0; i < 4096; i++) storage.set(i, (i * 7 + s) % 4);
            sections[s] = new Section(new Container(palettes[s % palettes.length], storage));
        }
        Weight[] weights = { new Flat(), new ByName() };
        System.out.println("ticked " + tick(sections, weights, 20000));

        // A value the palette lacks: the exception reaches the catch.
        Storage broken = new Storage(4, 4096);
        broken.set(5, 9);
        Section bad = new Section(new Container(new Linear(linearValues, 4), broken));
        Section[] withBad = { sections[0], bad };
        try {
            System.out.println("ticked " + tick(withBad, weights, 1000));
        } catch (MissingEntry e) {
            System.out.println("caught: " + e.getMessage());
        }
        Square[] squares = { new Square(2), new Square(5), new Square(7) };
        Shape[] shapes = { squares[0], new Circle(4), squares[2], new Circle(9) };
        System.out.println("areas " + areas(shapes, squares, 5000));
        Shape[] torn = { squares[1], new Torn() };
        for (int rounds = 1; rounds <= 3; rounds++) {
            try {
                System.out.println("areas " + areas(torn, squares, rounds));
            } catch (MissingEntry e) {
                System.out.println("caught: " + e.getMessage());
            }
        }
        System.out.println("done " + seed);
    }
}
