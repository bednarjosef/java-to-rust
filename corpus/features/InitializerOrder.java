// Field initializers and instance initializer blocks run in the order they are written, at the
// start of every constructor that does not call another one with `this(...)`.
public class InitializerOrder {
    static final StringBuilder LOG = new StringBuilder();

    // A reader in the style of Gson's JsonReader: a block between two groups of fields.
    static class Reader {
        private int[] stack = new int[32];
        private int stackSize = 0;

        {
            stack[stackSize++] = 6;
            LOG.append("block sees size ").append(stackSize).append('\n');
        }

        private String[] pathNames = new String[32];
        private int[] pathIndices = new int[32];
        private String name = "reader" + stackSize;

        Reader() {
            LOG.append("constructor sees " + name + " " + pathNames.length + " " + pathIndices.length).append('\n');
        }

        Reader(String first) {
            this();
            pathNames[0] = first;
        }
    }

    static int note(String what, int value) {
        LOG.append(what).append('\n');
        return value;
    }

    // Several blocks and fields interleaved, with side effects that show the order.
    static class Interleaved {
        int a = note("a", 1);
        { note("first block", 0); a += 10; }
        int b = note("b", a * 2);
        { note("second block", 0); b += a; }
        int c = note("c", a + b);
        String seen = "none";
        { seen = "a=" + a + " b=" + b + " c=" + c; }

        Interleaved() { note("constructor", 0); }
        Interleaved(int extra) { this(); c += extra; }
    }

    static class Base {
        int base = note("base field", 5);
        { note("base block", 0); }
        Base() { note("base constructor", 0); }
    }

    static class Derived extends Base {
        { note("derived block", 0); }
        int derived = note("derived field", base + 1);
        Derived() { super(); note("derived constructor " + derived, 0); }
    }

    // An inner class reads its enclosing instance in a field written after a block.
    int outerValue = 40;

    class Inner {
        int first = outerValue;
        { note("inner block " + first, 0); }
        int second = outerValue + first;
    }

    public static void main(String[] args) {
        Reader r = new Reader("root");
        System.out.println(r.stack[0] + " " + r.stackSize + " " + r.name + " " + r.pathNames[0]);
        Interleaved i = new Interleaved(100);
        System.out.println(i.a + " " + i.b + " " + i.c + " " + i.seen);
        Derived d = new Derived();
        System.out.println(d.base + " " + d.derived);
        Inner inner = new InitializerOrder().new Inner();
        System.out.println(inner.first + " " + inner.second);
        // An anonymous class captures a local in a field written after a block.
        int captured = args.length + 7;
        Object anonymous = new Object() {
            { note("anonymous block", 0); }
            int value = captured * 3;

            @Override
            public String toString() { return "anonymous " + value; }
        };
        System.out.println(anonymous);
        System.out.print(LOG);
    }
}
