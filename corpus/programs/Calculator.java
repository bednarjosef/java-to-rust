public class Calculator {
    static int[] stack = new int[16];
    static int top = 0;
    static int badChars = 0;

    static void push(int v) {
        stack[top++] = v;
    }

    static int pop() {
        return stack[--top];
    }

    static int apply(char op, int a, int b) {
        switch (op) {
            case '+':
                return a + b;
            case '-':
                return a - b;
            case '*':
                return a * b;
            default:
                return a / b;
        }
    }

    static String kind(char c) {
        return switch (c) {
            case '0', '1', '2', '3', '4', '5', '6', '7', '8', '9' -> "digit";
            case ' ' -> "space";
            case '+', '-', '*', '/' -> "operator";
            default -> "unknown";
        };
    }

    static int eval(String expr) {
        top = 0;
        int number = 0;
        boolean inNumber = false;
        for (int i = 0; i <= expr.length(); i++) {
            char c = i < expr.length() ? expr.charAt(i) : ' ';
            switch (kind(c)) {
                case "digit":
                    number = number * 10 + (c - '0');
                    inNumber = true;
                    break;
                case "space":
                    if (inNumber) {
                        push(number);
                        number = 0;
                        inNumber = false;
                    }
                    break;
                case "operator":
                    int b = pop();
                    int a = pop();
                    push(apply(c, a, b));
                    break;
                default:
                    badChars++;
                    break;
            }
        }
        return pop();
    }

    public static void main(String[] args) {
        String[] lines = {"3 4 + 2 *", "clear", "10 2 8 * + 3 -", "7 2 / x","9 4 - 3 *", "1 0 /", "5"};
        int done = 0;
        for (String line : lines) {
            switch (line) {
                case "clear" -> top = 0;
                default -> {
                    System.out.println(line + " = " + eval(line));
                    done++;
                }
            }
        }
        System.out.println("done " + done + ", bad characters " + badChars);
    }
}
