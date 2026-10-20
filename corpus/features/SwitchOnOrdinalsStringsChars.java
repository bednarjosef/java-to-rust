public class SwitchEnumsStrings {
    enum Op { ADD, SUB, MUL, NOP }

    static int apply(Op op, int a, int b) {
        switch (op.ordinal()) {
            case 0:
                return a + b;
            case 1:
                return a - b;
            case 2:
                return a * b;
            default:
                return a;
        }
    }

    static String kind(String word) {
        switch (word) {
            case "one":
            case "uno":
                return "number";
            case "red":
                return "color";
            case "":
                return "empty";
            default:
                return "unknown";
        }
    }

    static int weight(char c) {
        switch (c) {
            case 'a': case 'e': case 'i': case 'o': case 'u':
                return 1;
            case ' ':
                return 0;
            default:
                return Character.isDigit(c) ? 3 : 2;
        }
    }

    static int fallThrough(int n) {
        int r = 0;
        switch (n) {
            case 1:
                r += 1;
            case 2:
                r += 10;
                break;
            case 3:
                r += 100;
            default:
                r += 1000;
        }
        return r;
    }

    static String describe(Op op) {
        return switch (op.ordinal()) {
            case 0, 1 -> "additive";
            case 2 -> {
                String s = "multiplicative";
                yield s + "!";
            }
            case 3 -> "none";
            default -> "bad";
        };
    }

    static int score(String s) {
        return switch (s.length()) {
            case 0 -> -1;
            case 1, 2 -> 1;
            default -> {
                int total = 0;
                for (int i = 0; i < s.length(); i++) total += weight(s.charAt(i));
                yield total;
            }
        };
    }

    public static void main(String[] args) {
        Op[] ops = Op.values();
        for (Op op : ops) System.out.println(op + " " + apply(op, 6, 4) + " " + describe(op));
        System.out.println(kind("uno") + " " + kind("red") + " " + kind("") + " " + kind("x"));
        System.out.println(weight('e') + weight(' ') + weight('7') + weight('z'));
        System.out.println(fallThrough(1) + " " + fallThrough(2) + " " + fallThrough(3) + " " + fallThrough(9));
        System.out.println(score("") + " " + score("ab") + " " + score("a e7"));
        String word = "one";
        String out = switch (word) {
            case "one" -> "1";
            case "two" -> "2";
            default -> "?";
        };
        System.out.println(out);
        char grade = 'B';
        int points = switch (grade) {
            case 'A' -> 4;
            case 'B' -> 3;
            default -> 0;
        };
        System.out.println("points " + points);
        int code = 0;
        for (int i = 0; i < 6; i++) {
            switch (i % 3) {
                case 0 -> code += 1;
                case 1 -> { if (i > 2) code += 10; }
                default -> code += 100;
            }
        }
        System.out.println("code " + code);
    }
}
