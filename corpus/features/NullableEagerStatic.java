// A static of the main class that starts as an object but is later set to what may be null is
// an Option read as it is, not a placeholder read through java::non_null.
public class NullableEagerStatic {
    interface Named {
        String name();
    }
    static class Point implements Named {
        int x;
        Point(int x) {
            this.x = x;
        }
        public String name() {
            return "p" + x;
        }
        @Override
        public String toString() {
            return "Point(" + x + ")";
        }
    }
    static Named current = new Point(1);
    static boolean flag = false;

    static Named pick(int k) {
        return k > 2 ? new Point(k) : null;
    }

    public static void main(String[] args) {
        System.out.println("now " + current + " " + current.toString() + " " + current.name());
        current = flag ? new Point(2) : pick(1);
        System.out.println("then " + current + " " + (current == null));
        current = pick(5);
        System.out.println("last " + current + " " + current.name());
    }
}
