public class NullPointer {
    static String name(int n) {
        return n > 0 ? "n" + n : null;
    }

    static boolean same(String a, String b) {
        return a.equals(b);
    }

    public static void main(String[] args) {
        String s = name(3);
        System.out.println(s.length());
        System.out.println(s.equals("n3"));
        System.out.println("abc".equals(name(0)));
        System.out.println(same("x", "x"));
        boolean flag = s != null;
        String u = flag ? s : "x";
        System.out.println(u.length());
        System.out.println(name(0).length());
    }
}
