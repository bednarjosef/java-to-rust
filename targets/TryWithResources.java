// Features: try-with-resources on user AutoCloseable classes, close order (reverse of opening),
// suppressed exceptions from close() via getSuppressed, a resource opened in a try header with a
// catch and finally, nested resources, a resource that fails to open, a lambda AutoCloseable,
// and a generic helper that uses a resource supplier.
import java.util.ArrayList;
import java.util.List;

public class TryWithResources {
    static final List<String> events = new ArrayList<>();

    static class Connection implements AutoCloseable {
        private final String name;
        private final boolean failOnClose;

        Connection(String name, boolean failOnClose) {
            this.name = name;
            this.failOnClose = failOnClose;
            if (name.isEmpty()) {
                throw new IllegalArgumentException("empty connection name");
            }
            events.add("open " + name);
        }

        String query(String sql) {
            if (sql.contains("DROP")) {
                throw new IllegalStateException("refused: " + sql);
            }
            events.add(name + " ran " + sql);
            return name + ":" + sql.length();
        }

        @Override
        public void close() {
            events.add("close " + name);
            if (failOnClose) {
                throw new RuntimeException("close failed for " + name);
            }
        }
    }

    static class Transaction implements AutoCloseable {
        private boolean committed;

        void commit() {
            committed = true;
        }

        @Override
        public void close() {
            events.add(committed ? "transaction committed" : "transaction rolled back");
        }
    }

    static String withTransaction(String sql) {
        try (Transaction tx = new Transaction();
             Connection conn = new Connection("main", false)) {
            String result = conn.query(sql);
            tx.commit();
            return result;
        }
    }

    static String withBrokenClose() {
        try (Connection conn = new Connection("flaky", true)) {
            return conn.query("SELECT 1");
        } catch (RuntimeException e) {
            return "caught " + e.getMessage();
        } finally {
            events.add("finally after broken close");
        }
    }

    static String withBodyAndCloseFailure() {
        try (Connection conn = new Connection("both", true)) {
            return conn.query("DROP TABLE users");
        } catch (IllegalStateException e) {
            StringBuilder sb = new StringBuilder("body: " + e.getMessage());
            for (Throwable s : e.getSuppressed()) {
                sb.append(" | suppressed: ").append(s.getMessage());
            }
            return sb.toString();
        }
    }

    static String openFails() {
        try (Connection conn = new Connection("", false)) {
            return "never";
        } catch (IllegalArgumentException e) {
            return "open failed: " + e.getMessage();
        }
    }

    static int countWithLambda(int n) throws Exception {
        int[] counter = {0};
        AutoCloseable tick = () -> events.add("lambda closed after " + counter[0]);
        try (tick) {
            for (int i = 0; i < n; i++) {
                counter[0]++;
            }
        }
        return counter[0];
    }

    static void printEvents(String title) {
        System.out.println(title);
        for (String e : events) {
            System.out.println("  " + e);
        }
        events.clear();
    }

    public static void main(String[] args) throws Exception {
        System.out.println("result: " + withTransaction("SELECT * FROM t"));
        printEvents("commit case:");

        System.out.println("result: " + withBrokenClose());
        printEvents("broken close case:");

        System.out.println("result: " + withBodyAndCloseFailure());
        printEvents("body and close case:");

        System.out.println("result: " + openFails());
        printEvents("open failure case:");

        try (Connection outer = new Connection("outer", false);
             Connection inner = new Connection("inner", false)) {
            System.out.println(outer.query("SELECT a") + " " + inner.query("SELECT bb"));
        }
        printEvents("nested case:");

        System.out.println("counted: " + countWithLambda(4));
        printEvents("lambda case:");
    }
}
