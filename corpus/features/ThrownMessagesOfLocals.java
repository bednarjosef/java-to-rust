// Exceptions whose messages read locals of every primitive type, thrown where the method returns,
// inside loops, in `try` blocks whose handlers read the same locals afterwards, from lambdas, and
// with a `switch` or an `instanceof` in the message, and a range check as Apache Commons'
// `Validate.inclusiveBetween` writes it.
import java.util.function.IntUnaryOperator;

public class ThrownMessagesOfLocals {
    static void inclusiveBetween(long start, long end, long value) {
        if (value < start || value > end) {
            throw new IllegalArgumentException(String.format("The value %s is not in the specified inclusive range of %s to %s", value, start, end));
        }
    }

    static int checked(int index, int size) {
        inclusiveBetween(0L, size - 1, index);
        return index * 2;
    }

    static String everyType(int i, long l, double d, float f, char c, boolean b, byte y, short s) {
        try {
            if (i > 2) {
                throw new IllegalStateException("i=" + i + " l=" + l + " d=" + d + " f=" + f + " c=" + c + " b=" + b + " y=" + y + " s=" + s);
            }
            return "fine " + i;
        } catch (IllegalStateException e) {
            i += 100;
            return e.getMessage() + " then " + i + " " + l;
        }
    }

    static int inLoop(int[] values) {
        int sum = 0;
        for (int k = 0; k < values.length; k++) {
            int v = values[k];
            if (v < 0) {
                throw new IllegalArgumentException("negative " + v + " at " + k + " after " + sum);
            }
            sum += v;
        }
        return sum;
    }

    static String withSwitch(int kind, int n) {
        if (n > 3) {
            throw new IllegalArgumentException(switch (kind) {
                case 0 -> "zero " + n;
                default -> {
                    int doubled = n * 2;
                    yield "other " + doubled;
                }
            });
        }
        return "small " + n;
    }

    static String withPattern(Object o, int n) {
        if (n > 3) {
            throw new IllegalArgumentException(o instanceof Integer x ? "integer " + (x + n) : "other " + n);
        }
        return "small " + n;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 6; i++) {
            try {
                System.out.println(checked(i, 4));
            } catch (IllegalArgumentException e) {
                System.out.println(e.getMessage());
            }
        }
        for (int i = 1; i < 5; i++) {
            System.out.println(everyType(i, i * 1000000000000L, i / 3.0, i / 7f, (char) ('a' + i), i % 2 == 0, (byte) (i * 60), (short) (i * 9000)));
        }
        int[][] rows = {{1, 2, 3}, {4, -5, 6}, {-7}};
        for (int[] row : rows) {
            try {
                System.out.println(inLoop(row));
            } catch (IllegalArgumentException e) {
                System.out.println(e.getMessage());
            }
        }
        int limit = 10;
        IntUnaryOperator bounded = x -> {
            if (x > limit) {
                throw new ArithmeticException("over " + x + " by " + (x - limit));
            }
            return x + 1;
        };
        for (int x = 8; x < 13; x += 2) {
            try {
                System.out.println(bounded.applyAsInt(x));
            } catch (ArithmeticException e) {
                System.out.println(e.getMessage());
            }
        }
        for (int n = 2; n < 6; n += 3) {
            for (int kind = 0; kind < 2; kind++) {
                try {
                    System.out.println(withSwitch(kind, n) + " " + withPattern(kind == 0 ? Integer.valueOf(kind) : "s", n));
                } catch (IllegalArgumentException e) {
                    System.out.println(e.getMessage());
                }
                try {
                    System.out.println(withPattern(kind == 0 ? Integer.valueOf(kind + 40) : "s", n));
                } catch (IllegalArgumentException e) {
                    System.out.println(e.getMessage());
                }
            }
        }
    }
}
