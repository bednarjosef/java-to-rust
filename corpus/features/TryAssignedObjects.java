// Locals of a class, declared without a value and set by statements of a `try` block (and its
// `catch` clauses), then read after it, also in a loop: as javac's Main reads its arguments and
// the JDK's HashMap.clone its copy. The block carries their values out for the code after the
// `try`, where Rust would not see them set.
public class TryAssignedObjects {
    static class Box {
        final int v;
        Box(int v) {
            if (v < 0) throw new IllegalArgumentException("negative " + v);
            this.v = v;
        }
    }

    static int make(int v) {
        Box b;
        try {
            b = new Box(v);
        } catch (IllegalArgumentException e) {
            b = new Box(0);
        }
        return b.v;
    }

    static String both(int v, int w) {
        Box a;
        Box c;
        try {
            a = new Box(v);
            System.out.println("made " + a.v);
            c = new Box(w);
        } catch (IllegalArgumentException e) {
            return "failed: " + e.getMessage();
        }
        return a.v + " and " + c.v;
    }

    static int loop(int[] vs) {
        int sum = 0;
        Box b;
        for (int v : vs) {
            try {
                b = new Box(v);
            } catch (IllegalArgumentException e) {
                continue;
            }
            sum += b.v;
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(make(3) + " " + make(-2));
        System.out.println(both(1, 2));
        System.out.println(both(1, -2));
        System.out.println(both(-1, 2));
        System.out.println(loop(new int[] {1, -1, 5}));
    }
}
