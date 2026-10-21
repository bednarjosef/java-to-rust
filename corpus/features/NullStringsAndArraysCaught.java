// A NullPointerException from a dereference of a string or an array that may be null, which a
// catch or a finally sees: calls on a string, an array access, a store into an array, an array's
// length, and a string switch.
public class NullStringsAndArraysCaught {
    static String pick(int i) {
        return i > 0 ? "ack" : null;
    }

    static int[] numbers(int i) {
        return i > 0 ? new int[] {4, 5, 6} : null;
    }

    static String describe(String command) {
        try {
            return command.equals("ack") ? "acknowledged" : "other " + command.length();
        } catch (NullPointerException e) {
            return "no command";
        }
    }

    static int first(int[] values) {
        try {
            return values[0] + values.length;
        } catch (NullPointerException e) {
            return -1;
        }
    }

    static int store(int[] values) {
        try {
            values[1] = 9;
            return values[1];
        } catch (RuntimeException e) {
            return -2;
        }
    }

    static String trimmed(String s) {
        try {
            return "[" + s.trim().toUpperCase() + "]";
        } catch (NullPointerException e) {
            return "[]";
        } finally {
            System.out.println("trimmed " + (s != null));
        }
    }

    static int counted(String command) {
        int steps = 0;
        try {
            switch (command) {
                case "ack" -> steps = 1;
                default -> steps = 2;
            }
        } catch (NullPointerException e) {
            steps = -3;
        }
        return steps;
    }

    public static void main(String[] args) {
        for (int i = 1; i >= 0; i--) {
            System.out.println(describe(pick(i)) + " " + first(numbers(i)) + " " + store(numbers(i)));
            System.out.println(trimmed(pick(i)) + " " + counted(pick(i)));
        }
        int done = 0;
        try {
            try {
                System.out.println(pick(0).length());
            } finally {
                done++;
                System.out.println("finally " + done);
            }
        } catch (NullPointerException e) {
            System.out.println("caught after finally " + done);
        }
    }
}
