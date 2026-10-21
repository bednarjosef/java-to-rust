public class SuppressedExceptions {
    static class Connection implements AutoCloseable {
        private final String name;
        private final boolean failOnClose;

        Connection(String name, boolean failOnClose) {
            this.name = name;
            this.failOnClose = failOnClose;
            System.out.println("open " + name);
        }

        String query(String sql) {
            if (sql.startsWith("DROP")) {
                throw new IllegalStateException("refused " + sql);
            }
            return name + ": " + sql;
        }

        @Override
        public void close() {
            System.out.println("close " + name);
            if (failOnClose) {
                throw new IllegalArgumentException("close of " + name);
            }
        }
    }

    static String bodyAndCloseFail(String name) {
        try (Connection conn = new Connection(name, true)) {
            return conn.query("DROP TABLE users");
        } catch (IllegalStateException e) {
            StringBuilder sb = new StringBuilder("body: " + e.getMessage());
            for (Throwable s : e.getSuppressed()) {
                sb.append(" | suppressed: ").append(s.getMessage());
            }
            return sb.toString();
        }
    }

    static String closeFailsAlone() {
        try (Connection conn = new Connection("alone", true)) {
            return conn.query("SELECT 1");
        } catch (IllegalArgumentException e) {
            return "caught " + e.getMessage() + ", suppressed " + e.getSuppressed().length;
        }
    }

    static int closeFailsAfterReturn() {
        try (Connection conn = new Connection("returns", true)) {
            return 7;
        } catch (IllegalArgumentException e) {
            System.out.println("from close: " + e.getMessage() + ", suppressed " + e.getSuppressed().length);
            return -1;
        }
    }

    static void twoClosesFail() {
        try (Connection first = new Connection("first", true);
                Connection second = new Connection("second", true)) {
            throw new RuntimeException("body");
        } catch (RuntimeException e) {
            System.out.println(e.getMessage() + " " + e.getSuppressed().length);
            for (Throwable s : e.getSuppressed()) {
                System.out.println("  " + s);
            }
        }
    }

    static void addByHand() {
        Exception e = new Exception("main");
        System.out.println(e.getSuppressed().length);
        e.addSuppressed(new RuntimeException("first"));
        e.addSuppressed(new IllegalStateException("second"));
        for (Throwable s : e.getSuppressed()) {
            System.out.println(s);
        }
    }

    public static void main(String[] args) {
        System.out.println(bodyAndCloseFail("db"));
        System.out.println(closeFailsAlone());
        System.out.println(closeFailsAfterReturn());
        twoClosesFail();
        addByHand();
    }
}
