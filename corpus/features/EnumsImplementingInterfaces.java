public class EnumsImplementingInterfaces {
    interface Greeter {
        String who();
        default String greet() { return "hello " + who(); }
        static String twice(Greeter g) { return g.greet() + "; " + g.greet(); }
    }

    static class Person implements Greeter {
        String name;
        Person(String name) { this.name = name; }
        public String who() { return name; }
        void rename(String n) { name = n; }
    }

    interface Coded {
        int[] WEIGHTS = { 3, 1, 4 };
        int code();
        String name();
        default int weighted() { return code() * WEIGHTS[code() % WEIGHTS.length]; }
        default String label() { return name() + "#" + code(); }

        interface Visitor {
            String visit(Coded c);
        }
    }

    enum Color implements Coded {
        RED, GREEN, BLUE;
        public int code() { return ordinal() + 1; }
        public String toString() { return "color " + name().toLowerCase(); }
    }

    static class Holder {
        Greeter greeter;
        Coded coded;
    }

    static class Shout implements Coded.Visitor {
        public String visit(Coded c) { return c.label().toUpperCase(); }
    }

    public static void main(String[] args) {
        Person p = new Person("ann");
        Greeter g = p;
        System.out.println(g.greet());
        p.rename("bob");
        System.out.println(Greeter.twice(g));
        Holder h = new Holder();
        System.out.println(h.greeter == null);
        h.greeter = p;
        System.out.println(h.greeter.greet() + " " + (h.greeter == g));
        for (Color c : Color.values()) {
            Coded k = c;
            System.out.println(k.weighted() + " " + k.label() + " " + c.label() + " " + k + " " + c.name());
        }
        h.coded = Color.GREEN;
        System.out.println(h.coded.label() + " " + (h.coded == Color.GREEN) + " " + h.coded.name());
        Coded.Visitor v = new Shout();
        System.out.println(v.visit(Color.BLUE));
        System.out.println(Coded.WEIGHTS.length);
    }
}
