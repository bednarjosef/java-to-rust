public class Null {
    static String pick(boolean first, String a, String b) {
        return first ? a : b;
    }

    static String maybe(int n) {
        if (n > 2) {
            return "big" + n;
        }
        return null;
    }

    static int countNull(String a, String b) {
        int count = 0;
        if (a == null) count++;
        if (b != null) count++;
        return count;
    }

    static String describe(String s) {
        if (s != null) {
            return "len " + s.length();
        }
        return "none";
    }

    static int early(String s) {
        if (s == null) return -1;
        return s.length() + 1;
    }

    static String otherwise(String s) {
        if (s == null) {
            return "nil";
        } else {
            return s + "!";
        }
    }

    public static void main(String[] args) {
        String s = null;
        System.out.println(s);
        System.out.println("value: " + s);
        System.out.println(s == null);
        System.out.println(s != null);
        s = "hello";
        System.out.println(s);
        System.out.println(null == s);
        s = null;
        System.out.println(s == null);
        s += "!";
        System.out.println(s);
        String t = maybe(1);
        System.out.println(t);
        System.out.println(maybe(5));
        System.out.println(pick(false, "x", null));
        System.out.println(pick(true, null, "y"));
        System.out.println(countNull(null, "z"));
        System.out.println("abc".equals(null));
        System.out.println("abc".equals("abc"));
        System.out.println(s == null ? "none" : s);
        String u = countNull("a", null) > 5 ? "many" : null;
        System.out.println(u);
        System.out.println(describe(null));
        System.out.println(describe("abc"));
        System.out.println(early(null) + " " + early("ab"));
        System.out.println(otherwise(null) + " " + otherwise("q"));
    }
}
