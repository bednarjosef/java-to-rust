public class Strings {
    public static void main(String[] args) {
        String s = "abc";
        String t = s;
        s += "def";
        s += 1 + 2;
        s += 1.5;
        s += true;
        s = s + s;
        System.out.println(s + " " + t);
        System.out.println(1 + 2 + "x" + 1 + 2);
        System.out.println("x" + (1 + 2));
        String empty = "";
        System.out.println(empty.length() + " " + s.length() + " " + "h\u00e9llo".length());
        System.out.println(s.equals(t) + " " + t.equals("abc") + " " + "abc".equals(t));
        String built = "";
        for (int i = 0; i < 5; i++) {
            built += i;
            built += ",";
        }
        System.out.println(built);
        String multi = "line1\nline2";
        System.out.println(multi);
        long l = 5;
        double d = 0.5;
        System.out.println("long " + l + ", double " + d + ", sum " + (l + d) + ", neg " + -l);
    }
}
