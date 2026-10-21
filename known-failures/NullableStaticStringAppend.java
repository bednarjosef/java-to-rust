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
