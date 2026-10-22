// String.translateEscapes() on a malformed escape that nothing catches: it throws
// IllegalArgumentException, which ends the program with exit code 1, after the output before it.
// The `after` line is never printed.
public class TranslatedEscapesUncaught {
    public static void main(String[] args) {
        String text = "x\\" + "u0041";
        System.out.println("before");
        System.out.println(text.translateEscapes());
        System.out.println("after");
    }
}
