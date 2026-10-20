public class Lookup {
    static String root(int target) {
        for (int i = 1; i <= 20; i++) {
            if (i * i == target) {
                return "root " + i;
            }
        }
        return null;
    }

    static int lengthOr(String s, int fallback) {
        if (s == null) {
            return fallback;
        }
        return s.length();
    }

    static String describe(int target) {
        String found = root(target);
        if (found == null) {
            return "no root for " + target;
        }
        return found + " of " + target;
    }

    public static void main(String[] args) {
        String r = root(49);
        if (r != null) {
            System.out.println(r.length() + " " + r);
        }
        System.out.println(root(50));
        System.out.println("for 50: " + root(50));
        String missing = root(50);
        System.out.println(missing == null ? "missing" : missing);
        System.out.println(describe(16));
        System.out.println(describe(17));
        int total = 0;
        int misses = 0;
        for (int t = 1; t <= 100; t++) {
            String hit = root(t);
            if (hit != null) {
                total += hit.length();
            } else {
                misses++;
            }
        }
        System.out.println(total + " " + misses);
        System.out.println(lengthOr(root(4), -1) + " " + lengthOr(root(3), -1));
        System.out.println(lengthOr(root(1000), -2));
    }
}
