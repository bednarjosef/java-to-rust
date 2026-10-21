public class InnerClassTrees {
    interface Step {
        int run(int x);
        String name();
    }

    static class Labelled {
        final String label;
        Labelled(String label) { this.label = label; }
        String tag() { return "[" + label + "]"; }
    }

    static class Machine {
        int runs;
        int bias;
        final String id;

        Machine(String id, int bias) {
            this.id = id;
            this.bias = bias;
        }

        // An abstract inner class with two inner subclasses: one object of the outer class is
        // shared by the whole tree, held once.
        abstract class Op implements Step {
            final int arg;
            Op(int arg) { this.arg = arg; }
            abstract int apply(int x);
            public int run(int x) {
                runs++;
                return apply(x) + bias;
            }
        }

        class Add extends Op {
            Add(int arg) { super(arg); }
            int apply(int x) { return x + arg; }
            public String name() { return id + ".add" + arg; }
        }

        class Times extends Op {
            Times(int arg) { super(arg); }
            int apply(int x) { return x * arg; }
            public String name() { return id + ".times" + arg; }
        }

        // An inner class extending a static nested class.
        class Note extends Labelled {
            Note(String label) { super(label); }
            String text() { return tag() + " " + id + " ran " + runs; }
        }

        Step[] program() {
            return new Step[] { new Add(2), new Times(3), new Add(-1) };
        }
    }

    public static void main(String[] args) {
        Machine m = new Machine("m", 0);
        Machine n = new Machine("n", 10);
        Step[] steps = m.program();
        int x = 1;
        for (Step s : steps) {
            x = s.run(x);
            System.out.println(s.name() + " -> " + x);
        }
        Step other = n.new Times(2);
        System.out.println(other.name() + " -> " + other.run(x));
        Machine.Note note = m.new Note("done");
        System.out.println(note.text());
        System.out.println(n.new Note("other").text());
    }
}
