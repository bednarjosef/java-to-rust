// try-with-resources over an existing local variable, `try (r)`, which is `try (final var hidden = r)`:
// closed when the body completes, returns, breaks or throws, after the declared resources of the
// same statement (in the reverse of their order), in a loop on each pass, and not at all when the
// variable is null.
public class TryWithExistingLocal {
    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
            System.out.println("open " + name);
        }

        String read(int i) {
            if (i < 0) {
                throw new IllegalStateException(name + " cannot read " + i);
            }
            return name + "[" + i + "]";
        }

        @Override
        public void close() {
            System.out.println("close " + name);
        }
    }

    static String mixed(int i) {
        Resource shared = new Resource("shared" + i);
        try (Resource declared = new Resource("declared" + i); shared) {
            if (i == 0) {
                return declared.read(0);
            }
            return declared.read(i) + shared.read(i);
        } finally {
            System.out.println("finally " + i);
        }
    }

    static int loop(int n) {
        Resource tick = new Resource("tick");
        int count = 0;
        for (int i = 0; i < n; i++) {
            try (tick) {
                if (i == 1) {
                    continue;
                }
                if (i == 3) {
                    break;
                }
                count += tick.read(i).length();
            }
        }
        return count;
    }

    static void nothing() {
        Resource none = null;
        try (none) {
            System.out.println("body with no resource");
        }
    }

    public static void main(String[] args) {
        System.out.println(mixed(0));
        System.out.println(mixed(1));
        try {
            System.out.println(mixed(-1));
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println("count " + loop(5));
        nothing();
        Resource none = null;
        try (none) {
            throw new IllegalArgumentException("body fails with a null resource");
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
    }
}
