public class ObjectValues {
    static class Point {
        final int x, y;
        Point(int x, int y) { this.x = x; this.y = y; }
        @Override public boolean equals(Object o) {
            if (this == o) return true;
            if (!(o instanceof Point)) return false;
            Point p = (Point) o;
            return x == p.x && y == p.y;
        }
        @Override public int hashCode() { return 31 * x + y; }
        @Override public String toString() { return "(" + x + ", " + y + ")"; }
    }

    static class Plain {
        int n;
        Plain(int n) { this.n = n; }
    }

    enum Color { RED, GREEN }

    static String describe(Object o) {
        if (o == null) return "null";
        if (o instanceof String s) return "string of " + s.length();
        if (o instanceof Integer i) return "int " + (i + 1);
        if (o instanceof Double d) return "double " + d;
        if (o instanceof Point p) return "point " + p.x;
        if (o instanceof Color c) return "color " + c.ordinal();
        return "other";
    }

    static class Holder {
        Object value;
        Holder(Object value) { this.value = value; }
        boolean holds(Object other) { return value.equals(other); }
    }

    public static void main(String[] args) {
        Object a = new Point(1, 2);
        Object b = new Point(1, 2);
        System.out.println(a.equals(b) + " " + ((Point) a == (Point) b) + " " + a.hashCode());
        System.out.println(a);
        Object s = "text";
        Object n = 41;
        Object d = 2.5;
        Object c = Color.GREEN;
        System.out.println(describe(s) + "; " + describe(n) + "; " + describe(d) + "; " + describe(a) + "; " + describe(c) + "; " + describe(null));
        System.out.println(s.hashCode() + " " + n.hashCode() + " " + d.hashCode() + " " + s.equals("text") + " " + n.equals(41) + " " + n.equals(41L));
        String back = (String) s;
        int m = (Integer) n;
        System.out.println(back.length() + m);
        System.out.println(s.toString() + n.toString() + c);
        Holder h = new Holder(new Point(3, 4));
        System.out.println(h.holds(new Point(3, 4)) + " " + h.holds("x") + " " + h.value);
        h.value = "changed";
        System.out.println(h.value + " " + h.holds("changed"));
        Plain p = new Plain(7);
        Object q = p;
        System.out.println(q.equals(p) + " " + q.equals(new Plain(7)) + " " + (q.hashCode() == p.hashCode()) + " " + p.n);
        Point bad = (Point) s;
        System.out.println(bad);
    }
}
