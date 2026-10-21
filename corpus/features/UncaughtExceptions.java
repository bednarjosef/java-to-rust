// Exceptions that nothing catches end the program with exit code 1 (Java prints the exception and
// its stack trace to standard error): one thrown deep in a recursion, after printStackTrace of a
// caught exception with its cause, and a cause given to a constructor.
public class UncaughtExceptions {
    static class ConfigError extends RuntimeException {
        ConfigError(String message, Throwable cause) {
            super(message, cause);
        }
    }

    static int depth(int n) {
        if (n == 0) {
            throw new IllegalStateException("bottom reached");
        }
        return depth(n - 1) + 1;
    }

    static void load(String name) {
        try {
            if (name.length() > 3) {
                throw new IllegalArgumentException("name too long: " + name);
            }
            System.out.println("loaded " + name);
        } catch (IllegalArgumentException e) {
            throw new ConfigError("cannot load", e);
        }
    }

    public static void main(String[] args) {
        try {
            load("abcdef");
        } catch (ConfigError e) {
            e.printStackTrace();
            System.out.println("cause: " + e.getCause().getMessage());
        }
        Throwable chained = new RuntimeException(new IllegalArgumentException("inner"));
        System.out.println(chained.getMessage());
        System.out.println(chained);
        load("abc");
        System.out.println("depth " + depth(0 + 0 * 1) + " never");
    }
}
