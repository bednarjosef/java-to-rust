public class Families {
    // An immutable family: no field is written after construction.
    static abstract class Token {
        final String text;
        Token(String text) { this.text = text; }
        abstract int weight();
        public String toString() { return text + "/" + weight(); }
    }

    static class Word extends Token {
        Word(String text) { super(text); }
        int weight() { return text.length(); }
    }

    static class Number extends Token {
        final int value;
        Number(int value) { super("#" + value); this.value = value; }
        int weight() { return value; }
    }

    // A mutable family: a field of the base is written through the base type.
    static abstract class Counter {
        int count;
        String label;
        Counter(String label) { this.label = label; }
        abstract int step();
        void tick() { count += step(); }
        public String toString() { return label + "=" + count; }
    }

    static class Single extends Counter {
        Single() { super("single"); }
        int step() { return 1; }
    }

    static class Double extends Counter {
        int factor = 2;
        Double() { super("double"); }
        int step() { return factor; }
        public String toString() { return "x" + factor + " " + super.toString(); }
    }

    // A class whose objects are never used as their superclass stays a plain value.
    static class Box {
        int v;
        void add(int x) { v += x; }
    }

    static class Crate extends Box {
        int labels;
    }

    static class Holder {
        Counter current;
        Token last;
        Holder(Counter c) { current = c; }
    }

    static int total(Token[] tokens) {
        int sum = 0;
        for (Token t : tokens) sum += t.weight();
        return sum;
    }

    public static void main(String[] args) {
        Token[] tokens = { new Word("hello"), new Number(7), new Word("x") };
        System.out.println(total(tokens));
        for (Token t : tokens) System.out.println(t + " " + t.text);

        Counter[] counters = { new Single(), new Double() };
        for (int i = 0; i < 3; i++) {
            for (Counter c : counters) c.tick();
        }
        for (Counter c : counters) {
            c.count += 100;
            c.label = c.label.toUpperCase();
            System.out.println(c);
        }
        Holder h = new Holder(counters[1]);
        h.current.count -= 5;
        h.current.tick();
        System.out.println(h.current + " " + (h.last == null));
        h.last = tokens[1];
        System.out.println(h.last.weight());
        h.current = counters[0];
        h.current.count++;
        System.out.println(counters[0]);

        Crate cr = new Crate();
        cr.add(5);
        cr.labels = 2;
        Box b = new Box();
        b.add(1);
        System.out.println(cr.v + " " + cr.labels + " " + b.v);
    }
}
