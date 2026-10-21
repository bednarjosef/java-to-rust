// A class whose objects are never created still gets code for its equals and casts, which
// convert to Object; that code never runs, so it needs no Object variant for the class.
public class UncreatedClassEquals {
    static class Point {
        int x = 1;
    }
    static class Unused {
        int k;
        @Override
        public boolean equals(Object o) {
            if (this == o) {
                return true;
            }
            return o instanceof Unused u && u.k == k;
        }
        int twiceOf(Object o) {
            Unused u = (Unused) o;
            return u.k * 2;
        }
    }
    static Unused never = null;

    public static void main(String[] args) {
        Object text = "text";
        Object point = new Point();
        System.out.println(point.equals(text) + " " + text.equals(point));
        if (never != null) {
            System.out.println(never.equals(text) + " " + never.twiceOf(point));
        }
        Object nothing = null;
        Unused u = (Unused) nothing;
        System.out.println((u == null) + " " + (nothing instanceof Unused));
    }
}
