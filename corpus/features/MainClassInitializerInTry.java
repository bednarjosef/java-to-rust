// The static initializer of the main class may throw, and code inside a `try` reads its statics
// from a nested class: Java has run that initializer before `main`, so the `try` never sees it.
public class MainClassInitializerInTry {
    static int start = 3;
    static short[] table = new short[2];
    static int first = table[Math.floorMod(start, table.length)];
    static String log = "";

    static class Fail extends RuntimeException {
        Fail(String message) {
            super(message);
        }
    }

    static class Tagged {
        String tag = "tag" + start;
        int size = first + table.length;

        Tagged(int k) {
            if (k < 0) {
                throw new Fail("negative " + k);
            }
            size += k;
        }
    }

    static int bump(int k) {
        start += k;
        return start;
    }

    public static void main(String[] args) {
        for (int k = -1; k <= 1; k++) {
            try {
                Tagged t = new Tagged(k);
                System.out.println(t.tag + " " + t.size);
                log += bump(k) + " ";
            } catch (Fail e) {
                System.out.println("caught " + e.getMessage());
            }
        }
        System.out.println(log + first);
    }
}
