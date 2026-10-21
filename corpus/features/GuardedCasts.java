// A method that a `catch (NullPointerException ...)` observes: its `instanceof` tests and its
// early returns prove the values it then casts and calls not null.
public class GuardedCasts {
    static class Point {
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        int sum() {
            return x + y;
        }

        @Override
        public boolean equals(Object o) {
            if (o == this) {
                return true;
            }
            if (!(o instanceof Point)) {
                return false;
            }
            Point p = (Point) o;
            return p.x == x && p.y == y;
        }

        @Override
        public int hashCode() {
            return 31 * x + y;
        }
    }

    static int describe(Object o) {
        if (o == null) {
            return -1;
        }
        if (!(o instanceof Point)) {
            return 0;
        }
        return ((Point) o).sum();
    }

    static int firstSum(Point[] points) {
        for (Point p : points) {
            if (p == null) {
                continue;
            }
            return p.sum();
        }
        return -2;
    }

    static boolean same(Object a, Object b) {
        try {
            return a.equals(b);
        } catch (NullPointerException e) {
            return false;
        }
    }

    public static void main(String[] args) {
        Point a = new Point(1, 2);
        Point b = new Point(1, 2);
        Object text = "text";
        try {
            System.out.println(describe(a) + " " + describe(text) + " " + describe(null));
            System.out.println(firstSum(new Point[] {null, b}) + " " + firstSum(new Point[] {null}));
        } catch (NullPointerException e) {
            System.out.println("not reached");
        }
        System.out.println(same(a, b) + " " + same(a, text) + " " + same(a, null));
    }
}
