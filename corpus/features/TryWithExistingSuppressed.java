// try-with-resources over an existing variable whose `close` throws: when the body completes, the
// exception of `close` propagates; when the body throws too, the exception of `close` is added to
// the body's as suppressed, and the body's exception is the one thrown. A null variable is never
// closed, so no exception comes from it.
public class TryWithExistingSuppressed {
    static class Broken implements AutoCloseable {
        final String name;

        Broken(String name) {
            this.name = name;
        }

        @Override
        public void close() {
            System.out.println("close " + name);
            throw new IllegalStateException("close failed for " + name);
        }
    }

    static void body(Broken resource, boolean fail) {
        try (resource) {
            if (fail) {
                throw new RuntimeException("body failed for " + resource.name);
            }
            System.out.println("body done for " + resource.name);
        }
    }

    static void report(RuntimeException e) {
        System.out.println("caught " + e.getMessage());
        for (Throwable s : e.getSuppressed()) {
            System.out.println("  suppressed " + s.getMessage());
        }
    }

    public static void main(String[] args) {
        Broken first = new Broken("first");
        try {
            body(first, true);
        } catch (RuntimeException e) {
            report(e);
        }
        Broken second = new Broken("second");
        try {
            body(second, false);
        } catch (IllegalStateException e) {
            report(e);
        }
        Broken none = null;
        try (none) {
            System.out.println("null resource body");
        }
        try (first) {
            System.out.println("main body " + first.name);
        } catch (IllegalStateException e) {
            report(e);
        }
    }
}
