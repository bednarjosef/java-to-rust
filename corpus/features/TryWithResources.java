// try-with-resources over the program's own `AutoCloseable` classes: resources closed in the
// reverse of their order after the body, whether it completes, returns, breaks or throws; a
// `catch` and a `finally` that run after the resources are closed; a resource whose creation
// fails, which closes the ones before it; and a resource of a subclass with its own `close`.
public class TryWithResources {
    static int open = 0;

    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            if (name.isEmpty()) {
                throw new IllegalArgumentException("no name");
            }
            this.name = name;
            open++;
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
            open--;
            System.out.println("close " + name);
        }
    }

    static class Logged extends Resource {
        Logged(String name) {
            super(name);
        }

        @Override
        public void close() {
            System.out.println("logged " + name);
            super.close();
        }
    }

    static String first(String a, String b, int i) {
        try (Resource x = new Resource(a); Resource y = new Logged(b)) {
            if (i == 0) {
                return x.read(0);
            }
            return x.read(i) + y.read(i);
        } catch (IllegalArgumentException | IllegalStateException e) {
            System.out.println("failed: " + e.getMessage() + ", open " + open);
            return "-";
        } finally {
            System.out.println("done with " + a + ", open " + open);
        }
    }

    public static void main(String[] args) {
        System.out.println(first("a", "b", 1));
        System.out.println(first("c", "d", 0));
        System.out.println(first("e", "f", -1));
        System.out.println(first("g", "", 2));
        int count = 0;
        for (int i = 0; i < 5; i++) {
            try (Resource r = new Resource("loop" + i)) {
                if (i == 1) {
                    continue;
                }
                if (i == 3) {
                    break;
                }
                count += r.read(i).length();
            }
        }
        System.out.println(count + ", open " + open);
        try (Resource last = new Resource("last")) {
            System.out.println(last.read(-5));
        }
    }
}
