// Case labels that are constants rather than literals: static final fields, constants of other
// classes, and constant expressions that javac folds (MAX - 1, 'a' + 1, PREFIX + "x"). And
// switches on enums: statements with and without default, falling through, and switch
// expressions that name every constant.
public class SwitchConstants {
    static final int SMALL = 1;
    static final int LARGE = 100;
    static final char FIRST = 'a';
    static final String GREETING = "hello";
    static final String PREFIX = "pre";
    static final long BIG = 1L << 40;

    static class Codes {
        static final int OK = 200;
        static final int NOT_FOUND = 404;
        static final short TINY = 7;
    }

    enum Color { RED, GREEN, BLUE }

    static String size(int n) {
        switch (n) {
            case SMALL:
                return "small";
            case LARGE:
            case LARGE * 2:
                return "large";
            case LARGE - 1:
                return "almost large";
            case -SMALL:
                return "negative";
            case (int) BIG >>> 8:
                return "shifted";
            case ~SMALL:
                return "complement";
            default:
                return "other";
        }
    }

    static String status(int code) {
        return switch (code) {
            case Codes.OK -> "ok";
            case Codes.NOT_FOUND -> "not found";
            case Codes.OK + 1, Codes.OK + 2 -> "almost ok";
            case Codes.TINY -> "tiny";
            default -> "code " + code;
        };
    }

    static int letter(char c) {
        switch (c) {
            case FIRST:
                return 1;
            case FIRST + 1:
                return 2;
            case (char) (FIRST + 2):
                return 3;
            case 'z':
                return 26;
            default:
                return 0;
        }
    }

    static String word(String s) {
        switch (s) {
            case GREETING:
                return "greeting";
            case PREFIX + "fix":
                return "prefix";
            case PREFIX + 1:
                return "numbered";
            case "" + FIRST + 'b':
                return "letters";
            default:
                return "unknown";
        }
    }

    static String describe(Color c) {
        switch (c) {
            case RED:
                return "warm";
            case BLUE:
                return "cool";
            default:
                return "neutral";
        }
    }

    static int weight(Color c) {
        int w = 0;
        switch (c) {
            case RED:
                w += 1;
            case GREEN:
                w += 10;
                break;
            case BLUE:
                w += 100;
        }
        return w;
    }

    static String code(Color c) {
        return switch (c) {
            case RED -> "r";
            case GREEN -> "g";
            case BLUE -> {
                String b = "b";
                yield b + b;
            }
        };
    }

    public static void main(String[] args) {
        int[] values = {1, 100, 200, 99, -1, 0, -2, 5, 1 << 32};
        for (int v : values) {
            System.out.print(size(v) + " ");
        }
        System.out.println();
        System.out.println(status(200) + ", " + status(404) + ", " + status(201) + ", " + status(7) + ", " + status(500));
        System.out.println(letter('a') + " " + letter('b') + " " + letter('c') + " " + letter('z') + " " + letter('q'));
        System.out.println(word("hello") + " " + word("prefix") + " " + word("pre1") + " " + word("ab") + " " + word("x"));
        for (Color c : Color.values()) {
            System.out.println(c + ": " + describe(c) + " " + weight(c) + " " + code(c));
        }
        Color only = Color.GREEN;
        switch (only) {
            case GREEN -> System.out.println("green it is");
            default -> System.out.println("not green");
        }
        switch (only) {
            case RED:
                System.out.println("red");
        }
        System.out.println("done " + PREFIX + BIG + Codes.TINY + Color.valueOf("BLUE"));
    }
}
