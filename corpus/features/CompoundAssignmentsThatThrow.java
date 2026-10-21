// A compound assignment to an array element or a static field whose right operand calls a method
// that throws, where a caller catches it, as the JDK's `Collectors.summingInt` adds its mapper's
// value into `a[0]`. Java reads the old element before the call, so a call that changes the
// element does not change what is added to, and an exception leaves the element as it was.
public class CompoundAssignmentsThatThrow {
    static int calls;
    static int[] totals = new int[2];
    static int count;

    static int parse(String s) {
        calls++;
        return Integer.parseInt(s);
    }

    static int bump(int[] a, String s) {
        a[0] = 100;
        return parse(s);
    }

    static void add(int[] a, String s) {
        a[0] += parse(s);
        totals[1] += parse(s) * 2;
        count += parse(s);
        a[a.length - 1] -= bump(a, s);
    }

    public static void main(String[] args) {
        int[] a = {5, 1};
        try {
            add(a, "7");
            System.out.println(a[0] + " " + a[1] + " " + totals[1] + " " + count + " " + calls);
            add(a, "x");
        } catch (NumberFormatException e) {
            System.out.println("bad " + e.getMessage());
        }
        System.out.println(a[0] + " " + a[1] + " " + totals[1] + " " + count + " " + calls);
    }
}
