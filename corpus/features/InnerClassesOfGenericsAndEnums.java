public class InnerClassesOfGenericsAndEnums {
    static class Ring<X> {
        Object[] items = new Object[4];
        int count;
        void add(X x) { items[count++] = x; }
        String name() { return "ring"; }
        class Walker {
            int at;
            boolean more() { return at < count; }
            @SuppressWarnings("unchecked")
            X next() { return (X) items[at++]; }
            String name() { return "walker of " + Ring.this.name(); }
            void addMore(X x) { add(x); }
        }
        Walker walker() { return new Walker(); }
    }

    enum Mode {
        FAST, SLOW;
        class Meter {
            int ticks;
            String show() { return name() + " " + ticks + " " + ordinal(); }
        }
    }

    int total;
    void bump(int by) { total += by; }

    class Adder {
        int calls;
        void add(int x) {
            calls++;
            bump(x);
            if (total > 10) InnerClassesOfGenericsAndEnums.this.total = 0;
        }
    }

    static String run() {
        InnerClassesOfGenericsAndEnums e = new InnerClassesOfGenericsAndEnums();
        Adder a = e.new Adder();
        for (int i = 0; i < 6; i++) a.add(i);
        return e.total + " after " + a.calls;
    }

    public static void main(String[] args) {
        Ring<String> r = new Ring<>();
        r.add("a");
        r.add("b");
        Ring<String>.Walker w = r.walker();
        w.addMore("c");
        StringBuilder sb = new StringBuilder();
        while (w.more()) sb.append(w.next());
        System.out.println(sb + " " + w.name());
        Mode.Meter m = Mode.SLOW.new Meter();
        m.ticks = 3;
        System.out.println(m.show());
        System.out.println(run());
    }
}
