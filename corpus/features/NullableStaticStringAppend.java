// `+=` on a static `String` that may be null (it is assigned an element of a `String[]`):
// with itself and with a literal, appending to `null`.
public class NullableStaticStringAppend {
    static String s = "a";

    public static void main(String[] args) {
        String[] w = new String[2];
        s += s;
        System.out.println(s);
        s = w[1];
        s += "!";
        System.out.println(s);
    }
}
