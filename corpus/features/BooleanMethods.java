public class BooleanMethods {
    public static void main(String[] args) {
        // parseBoolean is true only for "true", in any case; no trimming and no null check.
        System.out.println(Boolean.parseBoolean("tRuE") + " " + Boolean.parseBoolean(" true") + " " + Boolean.parseBoolean("yes"));
        System.out.println(Boolean.parseBoolean("TRUE") + " " + Boolean.parseBoolean("false") + " " + Boolean.parseBoolean(""));
        System.out.println(Boolean.valueOf("True") + " " + Boolean.valueOf("1") + " " + Boolean.valueOf(false));

        // toString and compare: false is below true.
        System.out.println(Boolean.toString(false) + " " + Boolean.toString(true) + " " + Boolean.TRUE.toString());
        System.out.println(Boolean.compare(false, true) + " " + Boolean.compare(true, false) + " " + Boolean.compare(true, true));
        Boolean b = Boolean.FALSE;
        System.out.println(b.compareTo(Boolean.TRUE) + " " + b.hashCode() + " " + Boolean.TRUE.hashCode());

        // The logical operations evaluate both operands, as & | ^ do.
        boolean t = true;
        boolean f = false;
        System.out.println(Boolean.logicalAnd(t, f) + " " + Boolean.logicalAnd(t, t) + " " + Boolean.logicalAnd(f, f));
        System.out.println(Boolean.logicalOr(t, f) + " " + Boolean.logicalOr(f, f) + " " + Boolean.logicalOr(t, t));
        System.out.println(Boolean.logicalXor(t, f) + " " + Boolean.logicalXor(t, t) + " " + Boolean.logicalXor(f, f));
        boolean[] seen = new boolean[1];
        System.out.println(Boolean.logicalAnd(f, seen[0] = true) + " " + seen[0]);
        System.out.println(Boolean.logicalOr(t, seen[0] = false) + " " + seen[0]);

        // Boxed operands are unboxed.
        Boolean boxedTrue = Boolean.TRUE;
        System.out.println(Boolean.logicalXor(boxedTrue, f) + " " + boxedTrue.equals(t));
    }
}
