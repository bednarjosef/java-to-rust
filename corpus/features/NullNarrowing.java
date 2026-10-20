public class NullNarrowing {

    static String describe(String s) {
        if (s == null) {
            return "nothing";
        }
        return "text " + s;
    }

    static int len(String s) {
        return s != null && s.length() > 3 ? s.length() : -1;
    }

    static String orDefault(String missing) {
        return missing == null ? "missing" : missing;
    }

    static String find(int i) {
        if (i % 3 == 0) {
            return null;
        }
        return "item" + i;
    }

    static String noisy(String s) {
        System.out.println("evaluated " + s);
        return s;
    }

    public static void main(String[] args) {
        System.out.println(describe(null));
        System.out.println(describe("x"));
        System.out.println(len(null) + " " + len("ab") + " " + len("abcd"));
        System.out.println(orDefault(null) + " " + orDefault("here"));
        String s = find(1);
        if (s != null && s.length() == 5) {
            System.out.println("five: " + s);
        }
        int i = 1;
        String t = find(i);
        while (t != null && t.length() < 10) {
            System.out.println("got " + t);
            i++;
            if (i > 4) {
                break;
            }
        }
        String n = find(3);
        System.out.println("x".equals(noisy("x")));
        System.out.println(n.equals(noisy("y")));
    }
}
