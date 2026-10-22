import java.util.ArrayList;
import java.util.List;
import java.util.function.IntSupplier;

/**
 * Values that the walk of the methods that can run knows: a parameter that every call passes the
 * same constant, a field that every constructor sets alike, a class with no objects yet, and the
 * places whose values it must not take as known (a record's fields, captured locals, a field or a
 * static written later, a subclass that passes another constant).
 */
public class ConstantArguments {
    record Flags(boolean verbose, int level) {}

    static class Task {
        final boolean parallel;
        final int depth;
        boolean done;

        Task(boolean parallel, int depth) {
            this.parallel = parallel;
            this.depth = depth;
        }

        String run() {
            if (parallel && depth < 3) {
                return "forked " + depth;
            }
            done = true;
            return "ran " + depth;
        }
    }

    static class Special extends Task {
        Special() {
            super(true, 1);
        }
    }

    static class Later {
        String hello() {
            return "later";
        }
    }

    static Object made;

    static String multiply(int n, boolean parallel, int depth) {
        if (depth > 3) {
            return "deep " + parallel;
        }
        Task t = new Task(parallel, depth);
        return t.run() + " / " + multiply(n, parallel, depth + 1);
    }

    static String describe(Object o) {
        if (o instanceof Later l) {
            return l.hello();
        }
        return "not later";
    }

    static boolean flag;

    static int counter(int start) {
        int i = start;
        int total = 0;
        while (i < 5) {
            total += i;
            i++;
        }
        return total;
    }

    public static void main(String[] args) {
        System.out.println(multiply(3, false, 0));
        // A record's fields come from its canonical constructor.
        Flags f = new Flags(true, 2);
        if (f.verbose()) {
            System.out.println("verbose " + f.level());
        }
        Flags g = new Flags(false, 0);
        System.out.println(g.verbose() ? "g verbose" : "g quiet");
        // A captured local in a lambda and in a local class.
        boolean captured = args.length == 0;
        IntSupplier s = () -> captured ? 1 : 2;
        System.out.println("supplier " + s.getAsInt());
        class Local {
            String show() {
                return captured ? "captured true" : "captured false";
            }
        }
        System.out.println(new Local().show());
        // An instanceof whose class has no objects until later.
        System.out.println(describe("text"));
        made = new Later();
        System.out.println(describe(made));
        // A field written after its first read.
        Task t = new Task(false, 9);
        System.out.println(t.done ? "done early" : "not done");
        System.out.println(t.run());
        System.out.println(t.done ? "done" : "still not done");
        // A subclass that passes a different constant.
        System.out.println(new Special().run());
        // A static written later.
        System.out.println(flag ? "flag" : "no flag");
        flag = true;
        System.out.println(flag ? "flag" : "no flag");
        System.out.println("counter " + counter(1));
        List<Boolean> list = new ArrayList<>();
        list.add(false);
        list.add(true);
        for (boolean b : list) {
            System.out.println(b ? "yes" : "no");
        }
    }
}
