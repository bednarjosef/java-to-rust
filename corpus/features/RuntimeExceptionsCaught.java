// Failures of the runtime caught as exceptions: a division by zero (also of a `long`, and in a
// compound assignment), an index out of bounds (read, written, updated and incremented, on a
// local array and on a field), `charAt` and `substring` out of range, and malformed numbers for
// `parseInt` (with a radix), `parseLong` and `parseDouble`. They are caught where they happen,
// in a caller, by a superclass (`RuntimeException`, `Exception`), by a multi-catch, and seen by a
// `finally` on their way to an uncaught end. A `split` with a literal pattern cannot fail.
public class RuntimeExceptionsCaught {
    static int ratio(int a, int b) {
        int r = a;
        r /= b;
        return r;
    }

    static int safeRatio(int a, int b) {
        try {
            return ratio(a, b);
        } catch (ArithmeticException | NumberFormatException e) {
            System.out.println("no ratio: " + e.getMessage());
            return 0;
        }
    }

    static int parse(String s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            System.out.println("bad number: " + e.getMessage());
            return -1;
        }
    }

    static int element(int[] values, int i) {
        try {
            return values[i];
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println(e.getMessage());
            return 0;
        } finally {
            System.out.println("looked up " + i);
        }
    }

    static char letter(String s, int i) {
        try {
            return s.charAt(i);
        } catch (StringIndexOutOfBoundsException e) {
            System.out.println("no letter: " + e.getMessage());
            return '?';
        }
    }

    static int countDown(int n) {
        int steps = 0;
        try {
            while (100 / n > 0) {
                n--;
                steps++;
            }
        } catch (Exception e) {
            System.out.println("stopped after " + steps + ": " + e);
        }
        return steps;
    }

    static int[] slots = new int[2];

    static void write(int[] a, int i) {
        try {
            a[i] = 5;
            System.out.println("wrote " + i);
            a[i] += 2;
            a[i]++;
            slots[i] = a[i];
            slots[i]--;
            System.out.println("slot " + slots[i]);
            System.out.println("before " + a[i]++ + " after " + ++a[i]);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("no slot: " + e.getMessage());
        }
    }

    static int sum(String[] parts) {
        int total = 0;
        for (String p : parts) {
            try {
                total += Integer.parseInt(p.trim());
            } catch (NumberFormatException e) {
                System.out.println("skip '" + p + "'");
                continue;
            } finally {
                System.out.println("after '" + p + "'");
            }
            System.out.println("total " + total);
        }
        return total;
    }

    public static void main(String[] args) {
        int[] divisors = {2, 0, 5};
        for (int d : divisors) {
            try {
                System.out.println(10 / d);
            } catch (ArithmeticException e) {
                System.out.println("caught " + e);
            }
        }
        System.out.println(safeRatio(9, 3) + safeRatio(1, 0));
        System.out.println(parse("42") + parse("4x2") + parse(""));
        int[] values = {7, 8, 9};
        System.out.println(element(values, 1));
        System.out.println(element(values, 3));
        System.out.println(letter("abc", 1));
        System.out.println(letter("abc", 5));
        System.out.println(countDown(3));
        System.out.println(sum(" 4,x,5 ".split(",")));
        write(values, 1);
        write(values, 2);
        write(values, -1);
        try {
            String word = "hello";
            System.out.println(word.substring(2, 10));
        } catch (RuntimeException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
        try {
            long big = Long.parseLong("123456789012");
            System.out.println(big % (big - big));
        } catch (ArithmeticException e) {
            System.out.println("long " + e.getMessage());
        }
        try {
            System.out.println(Integer.parseInt("ff", 16) + Integer.parseInt("zz", 10));
        } catch (NumberFormatException e) {
            System.out.println(e.getMessage());
        }
        double d = Double.parseDouble("2.5");
        try {
            System.out.println(d + Double.parseDouble("2.5x"));
        } catch (NumberFormatException e) {
            System.out.println(e.getMessage());
        }
        int total = 0;
        try {
            for (int i = 0; i <= values.length; i++) {
                total += values[i];
            }
        } finally {
            System.out.println("total " + total);
        }
    }
}
