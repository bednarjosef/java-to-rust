// Exceptions with causes: a failure wrapped in an exception of the program with
// `super(message, cause)`, one wrapped with only a cause (whose message is the cause's
// `toString`), a chain walked with `getCause`, a runtime failure as the cause, and
// `printStackTrace` of a chain (to standard error, without stack frames).
public class ExceptionCauses {
    static class ConfigException extends Exception {
        final String key;

        ConfigException(String key, Throwable cause) {
            super("bad value for " + key, cause);
            this.key = key;
        }
    }

    static int parsePort(String text) {
        int port = Integer.parseInt(text);
        if (port <= 0 || port > 65535) {
            throw new IllegalArgumentException("port " + port + " out of range");
        }
        return port;
    }

    static int port(String key, String text) throws ConfigException {
        try {
            return parsePort(text);
        } catch (RuntimeException e) {
            throw new ConfigException(key, e);
        }
    }

    static int ratio(int a, int b) {
        try {
            return a / b;
        } catch (ArithmeticException e) {
            throw new IllegalStateException(e);
        }
    }

    static void describe(Throwable t) {
        int depth = 0;
        for (Throwable c = t; c != null; c = c.getCause()) {
            System.out.println("  ".repeat(depth) + c.getClass().getSimpleName() + ": " + c.getMessage());
            depth++;
        }
    }

    public static void main(String[] args) {
        String[][] settings = {{"http", "8080"}, {"ssh", "x22"}, {"ftp", "70000"}};
        for (String[] s : settings) {
            try {
                System.out.println(s[0] + " on " + port(s[0], s[1]));
            } catch (ConfigException e) {
                System.out.println(e.key + " -> " + e.getMessage());
                describe(e);
                System.out.println("caused by " + e.getCause());
            }
        }
        try {
            System.out.println(ratio(6, 3));
            System.out.println(ratio(1, 0));
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
            System.out.println(e.getCause().getMessage());
            describe(e);
            e.printStackTrace();
        }
        RuntimeException plain = new RuntimeException("plain");
        System.out.println(plain.getCause() == null);
        Exception wrapped = new Exception(plain);
        System.out.println(wrapped);
        System.out.println(wrapped.getCause() == plain);
    }
}
