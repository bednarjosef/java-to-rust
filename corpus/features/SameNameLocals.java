import java.util.function.Supplier;

// Locals of different types that share a name in scopes that do not overlap: loop variables,
// block locals, catch parameters and a pattern binding, one of them null and one captured by a
// lambda. Each keeps its own type and nullness.
public class SameNameLocals {
    static class Box {
        int v;

        Box(int v) {
            this.v = v;
        }
    }

    static int get(Box b) {
        return b.v;
    }

    static String show(Object o) {
        if (o instanceof String s) {
            return "text " + s.length();
        }
        {
            Box s = new Box(7);
            return "box " + get(s);
        }
    }

    public static void main(String[] args) {
        Box[] boxes = {new Box(1), new Box(2)};
        for (Box s : boxes) {
            System.out.printf("%d%n", get(s));
        }
        for (String s : new String[] {null, "a"}) {
            System.out.println(s);
        }
        {
            int x = 3;
            System.out.println(x + 1);
        }
        {
            String x = args.length > 5 ? "many" : null;
            System.out.println(x == null ? "none" : x);
            Supplier<String> later = () -> "later " + (x == null);
            System.out.println(later.get());
        }
        try {
            System.out.println(boxes[2].v);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("out of bounds");
        }
        try {
            Object o = "x";
            System.out.println(((Box) o).v);
        } catch (ClassCastException e) {
            System.out.println("not a box");
        }
        System.out.println(show("abc") + ", " + show(3));
    }
}
