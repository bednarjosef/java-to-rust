// Unboxing a null box throws a `NullPointerException` with Java's message wherever the box is
// converted to a primitive, which the code mostly does not write as a cast: a value returned,
// assigned, passed or initializing a local, an arm of `?:` and of a switch expression, an
// operand, an index, a size, an element, and a boxed local that `++` or `+=` updates.
public class UnboxingNullMessages {
    final int value;

    UnboxingNullMessages(int value) {
        this.value = value;
    }

    static Integer missing(int n) {
        return n > 100 ? n : null;
    }

    static int returned(Integer x) {
        return x;
    }

    static long widened(Integer x) {
        return x;
    }

    static int plusOne(Integer x) {
        return x + 1;
    }

    static void takesInt(int i) {
        System.out.println(i);
    }

    static Character noChar(int n) {
        return n > 100 ? 'c' : null;
    }

    static Boolean noBoolean(int n) {
        return n > 100 ? Boolean.TRUE : null;
    }

    // Boxes tested not to be null are unboxed without a check: in the arm of `?:` that the
    // condition guards, and in a case after the `if` that leaves for a null.
    static int day(Integer value) {
        return (value != null && value >= 1) ? value : 1;
    }

    static int sum(int a, boolean b) {
        return b ? a : -a;
    }

    static int inCase(int k, Integer number) {
        switch (k) {
            case 1:
                if (number == null)
                    throw new IllegalArgumentException("no number");
                return sum(number, k > 0);
            default:
                return 0;
        }
    }

    public static void main(String[] args) {
        int n = args.length;
        Integer missing = missing(n);
        int[] numbers = {1, 2};
        Integer two = n + 2;
        System.out.println(returned(two) + " " + widened(two) + " " + numbers[two - 1]);
        try {
            System.out.println(day(missing) + " " + day(two) + " " + inCase(1, two) + " " + inCase(1, missing));
        } catch (NullPointerException | IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        try {
            returned(null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(widened(missing));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(plusOne(null));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = missing(n);
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = missing;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z;
            z = missing;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            takesInt(missing);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(new UnboxingNullMessages(missing).value);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = n > 5 ? 1 : missing;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = switch (n) {
                case 0 -> missing;
                default -> 2;
            };
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println((int) missing);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(-missing);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(missing < 3);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(missing == 3);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(numbers[missing]);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(new int[missing].length);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int[] made = {missing};
            System.out.println(made.length);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            numbers[0] = missing;
            System.out.println(numbers[0]);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = 1;
            z += missing;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            Integer w = missing;
            w++;
            System.out.println(w);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            Integer w = missing;
            ++w;
            System.out.println(w);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            Integer w = missing;
            w += 2;
            System.out.println(w);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            Integer w = missing;
            int z = w++ + 1;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            char c = noChar(n);
            System.out.println(c);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            boolean b = noBoolean(n);
            System.out.println(b);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
