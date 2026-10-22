// Locals of an enum and of a box, declared without a value and assigned in a `try` block and its
// `catch` clauses, then read after it: as in javac's JavacParser (a `Float`) and the JDK's
// MessageFormat (an enum). Rust cannot tell they are assigned, so they start as a value that
// Java's definite assignment never lets the code read.
public class TryAssignedEnumsAndBoxes {
    enum Kind { SMALL, LARGE }
    static float parse(String s) {
        Float n;
        try {
            n = Float.valueOf(s);
        } catch (NumberFormatException ex) {
            n = Float.NaN;
        }
        return n.floatValue() * 2;
    }
    static String kind(String s) {
        Kind k;
        try {
            k = Kind.valueOf(s.trim().toUpperCase());
        } catch (IllegalArgumentException e) {
            throw new IllegalArgumentException("unknown kind: " + s);
        }
        return k.name() + " " + k.ordinal();
    }
    public static void main(String[] args) {
        System.out.println(parse("1.5") + " " + parse("x"));
        System.out.println(kind(" large "));
        try { kind("medium"); } catch (IllegalArgumentException e) { System.out.println(e.getMessage()); }
    }
}
