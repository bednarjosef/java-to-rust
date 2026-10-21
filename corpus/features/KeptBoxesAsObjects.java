// Boxes whose identity the program compares keep it (`java::Boxed`), also as `Object`s: `Object`'s
// equality of two of them is identity, and `equals` compares their values.
public class KeptBoxesAsObjects {
    static String describe(Object o) {
        return o + (o instanceof Boolean ? " (a Boolean)" : " (an Integer)");
    }

    static boolean same(Object a, Object b) {
        return a.equals(b);
    }

    public static void main(String[] args) {
        Boolean t = Boolean.valueOf(args.length == 0);
        Boolean u = Boolean.TRUE;
        System.out.println(t == u);
        Integer big = 1000;
        Integer other = 1000;
        Integer small = 100;
        Integer alsoSmall = 100;
        System.out.println((big == other) + " " + (small == alsoSmall) + " " + big.equals(other));
        Object[] values = {t, u, big, other, false};
        for (Object v : values) {
            System.out.println(describe(v));
        }
        System.out.println(same(t, u) + " " + same(big, other) + " " + same(big, t));
    }
}
