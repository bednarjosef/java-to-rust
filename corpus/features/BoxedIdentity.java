public class BoxedIdentity {
    static Integer shared = 127;
    Integer count;
    Long total;

    static boolean same(Integer a, Integer b) {
        return a == b;
    }

    public static void main(String[] args) {
        Integer a = 100;
        Integer b = 100;
        System.out.println("100: " + (a == b) + " " + a.equals(b));

        Integer c = 1000;
        Integer d = 1000;
        System.out.println("1000: " + (c == d) + " " + c.equals(d));

        Integer e = Integer.valueOf(127);
        System.out.println("127: " + (e == shared) + " " + (e == 127));
        System.out.println("same: " + same(a, b) + " " + same(c, d) + " " + same(c, c));

        Long la = 127L;
        Long lb = Long.valueOf(127);
        Long lc = 128L;
        Long ld = 128L;
        System.out.println("long: " + (la == lb) + " " + (lc == ld) + " " + lc.equals(ld));

        Character ca = 'z';
        Character cb = Character.valueOf('z');
        Character cc = (char) 200;
        Character cd = (char) 200;
        System.out.println("char: " + (ca == cb) + " " + (cc == cd) + " " + cc.equals(cd));

        Boolean t1 = true;
        Boolean t2 = Boolean.valueOf(true);
        Boolean f1 = Boolean.valueOf("no");
        System.out.println("bool: " + (t1 == t2) + " " + (f1 == Boolean.FALSE) + " " + f1.equals(false));

        Short s1 = 5;
        Short s2 = 5;
        Byte y1 = 5;
        Byte y2 = Byte.valueOf((byte) 5);
        System.out.println("small: " + (s1 == s2) + " " + (y1 == y2));

        Double x1 = 2.5;
        Double x2 = 2.5;
        System.out.println("double: " + (x1 == x2) + " " + x1.equals(x2));

        BoxedIdentity holder = new BoxedIdentity();
        System.out.println("field null: " + (holder.count == null));
        holder.count = 300;
        holder.total = 300L;
        Integer seen = holder.count;
        System.out.println("field: " + holder.count + " " + (seen == holder.count) + " " + (holder.count == 300));
        holder.count = null;
        System.out.println("cleared: " + holder.count);

        Integer nothing = null;
        System.out.println("nothing: " + nothing + " " + (nothing == null) + " " + (nothing != a));
        System.out.println("compare: " + a.compareTo(c) + " " + c.compareTo(a) + " " + a.compareTo(b));
        System.out.println("cmp: " + Integer.compare(3, 9) + " " + Long.compare(9L, 3L) + " " + Double.compare(1.0, 1.0));
        System.out.println("hash: " + a.hashCode() + " " + la.hashCode() + " " + x1.hashCode() + " " + t1.hashCode());
    }
}
