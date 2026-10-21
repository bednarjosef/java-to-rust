// A calculator for expressions in reverse Polish notation. Each line is split into tokens; a
// number is parsed with `Integer.parseInt`, whose malformed input is reported, and an operator
// applies to the top of a stack, where a division by zero and a stack with too few values are
// errors of the line. A `finally` counts the lines however they end.
public class RpnCalculatorErrors {
    static class CalculationError extends RuntimeException {
        final int position;

        CalculationError(String message, int position) {
            super(message);
            this.position = position;
        }
    }

    static final int CAPACITY = 8;

    static int[] stack = new int[CAPACITY];
    static int size = 0;
    static int lines = 0;
    static int failures = 0;

    static void push(int value, int position) {
        if (size == CAPACITY) {
            throw new CalculationError("stack full", position);
        }
        stack[size] = value;
        size++;
    }

    static int pop(int position) {
        if (size == 0) {
            throw new CalculationError("missing operand", position);
        }
        size--;
        return stack[size];
    }

    static int apply(char op, int a, int b) {
        switch (op) {
            case '+':
                return a + b;
            case '-':
                return a - b;
            case '*':
                return a * b;
            case '/':
                return a / b;
            case '%':
                return a % b;
            default:
                throw new IllegalArgumentException("unknown operator " + op);
        }
    }

    static int evaluate(String line) {
        size = 0;
        String[] tokens = line.trim().split(" +");
        for (int i = 0; i < tokens.length; i++) {
            String token = tokens[i];
            if (token.length() == 1 && "+-*/%".indexOf(token.charAt(0)) >= 0) {
                int b = pop(i);
                int a = pop(i);
                push(apply(token.charAt(0), a, b), i);
            } else {
                push(Integer.parseInt(token), i);
            }
        }
        int result = pop(tokens.length);
        if (size != 0) {
            throw new CalculationError(size + " values left over", tokens.length);
        }
        return result;
    }

    static void run(String line) {
        try {
            System.out.println(line + " = " + evaluate(line));
        } catch (CalculationError e) {
            failures++;
            System.out.println(line + ": " + e.getMessage() + " at token " + e.position);
        } catch (ArithmeticException e) {
            failures++;
            System.out.println(line + ": " + e.getMessage());
        } catch (NumberFormatException e) {
            failures++;
            System.out.println(line + ": not a number (" + e.getMessage() + ")");
        } finally {
            lines++;
        }
    }

    public static void main(String[] args) {
        String[] input = {
            "3 4 +",
            "5 1 2 + 4 * + 3 -",
            "2 3 4 * +",
            "10 0 /",
            "7 0 %",
            "1 +",
            "1 2 3",
            "12 x3 *",
            "1 2 3 4 5 6 7 8 9 + + + + + + + +",
            "2147483647 1 +",
            "-6 4 /",
            "  8   2   /  ",
        };
        for (String line : input) {
            run(line);
        }
        System.out.println(lines + " lines, " + failures + " failed");
    }
}
