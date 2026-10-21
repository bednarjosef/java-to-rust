// Locals declared in the case blocks of switch expressions that may hold null: one read after a
// null check, one assigned in a loop inside a case, and one in a switch expression nested in a case
// block of another. Their nullness is found only if the analyses see the statements of the cases.
public class SwitchExpressionBlockLocals {
    static String mayBeNull(int i) {
        return i % 2 == 0 ? null : "value" + i;
    }

    static int length(int i) {
        return switch (i % 4) {
            case 0 -> 0;
            case 1 -> {
                String s = mayBeNull(i);
                yield s == null ? -1 : s.length();
            }
            default -> {
                String t = mayBeNull(i + 1);
                yield t == null ? -2 : t.length();
            }
        };
    }

    static String lastNonNull(int n) {
        return switch (n) {
            case 0 -> "none";
            default -> {
                String last = null;
                for (int i = 1; i <= n; i++) {
                    last = mayBeNull(i);
                }
                yield last == null ? "null" : last;
            }
        };
    }

    static String nested(int a, int b) {
        return switch (a) {
            case 1 -> {
                String outer = mayBeNull(b);
                yield switch (b % 3) {
                    case 0 -> {
                        String inner = mayBeNull(b);
                        yield inner == null ? "inner null, outer " + outer : inner + " " + outer;
                    }
                    default -> outer == null ? "outer null" : outer;
                };
            }
            default -> "other";
        };
    }

    public static void main(String[] args) {
        for (int i = 0; i < 6; i++) {
            System.out.println("length " + i + " = " + length(i));
        }
        System.out.println(lastNonNull(0));
        System.out.println(lastNonNull(1));
        System.out.println(lastNonNull(4));
        for (int b = 0; b < 4; b++) {
            System.out.println("nested " + b + ": " + nested(1, b) + " / " + nested(2, b));
        }
    }
}
