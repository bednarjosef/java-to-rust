public class AnonymousClassPlaces {
    interface Task { String run(); }

    static abstract class Base {
        final int n;
        Base(int n) { this.n = n; }
        abstract int value();
    }

    static Task shared = new Task() {
        public String run() { return "static task"; }
    };

    String label = "obj";
    Task own = new Task() {
        int runs;
        public String run() { runs++; return label + " task " + runs; }
    };

    static String loops(String[] names) {
        Task[] tasks = new Task[names.length];
        int i = 0;
        for (String name : names) {
            int at = i;
            tasks[i++] = new Task() {
                public String run() { return at + "=" + name; }
            };
        }
        StringBuilder sb = new StringBuilder();
        for (Task t : tasks) sb.append(t.run()).append(' ');
        return sb.toString();
    }

    static String caught() {
        try {
            throw new IllegalStateException("boom");
        } catch (IllegalStateException e) {
            String what = e.getMessage();
            class Report { String text() { return "caught " + what; } }
            return new Report().text();
        }
    }

    static int chained(int k) {
        int base = k * 10;
        class Counter extends Base {
            Counter(int n) { super(n); }
            int value() { return base + n; }
        }
        Base b = new Counter(3) {
            int value() { return super.value() * 2 + k; }
        };
        Base c = new Counter(4);
        return b.value() + c.value();
    }

    static String nestedAnon(String word) {
        Task outer = new Task() {
            public String run() {
                Task inner = new Task() {
                    public String run() { return word + "!"; }
                };
                return "<" + inner.run() + ">";
            }
        };
        return outer.run();
    }

    public static void main(String[] args) {
        System.out.println(shared.run());
        AnonymousClassPlaces e = new AnonymousClassPlaces();
        System.out.println(e.own.run());
        e.label = "changed";
        System.out.println(e.own.run());
        System.out.println(loops(new String[] { "x", "y", "z" }));
        System.out.println(caught());
        System.out.println(chained(2));
        System.out.println(nestedAnon("hey"));
        System.out.println(e.own.getClass().getName() + " " + shared.getClass().getName());
    }
}
