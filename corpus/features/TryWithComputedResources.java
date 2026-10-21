// try-with-resources whose resource is not a `new` object: one that a method returns, which may
// be null (Java then closes nothing), and one of an interface type that extends `AutoCloseable`,
// as the JDK's `flatMap` closes each stream its mapper returns.
public class TryWithComputedResources {
    interface Channel extends AutoCloseable {
        String read();

        @Override
        void close();
    }

    static final StringBuilder log = new StringBuilder();

    static final class Pipe implements Channel {
        final String name;

        Pipe(String name) {
            this.name = name;
            log.append("open ").append(name).append("; ");
        }

        public String read() {
            if (name.equals("bad")) {
                throw new IllegalStateException("cannot read " + name);
            }
            return name.toUpperCase();
        }

        public void close() {
            log.append("close ").append(name).append("; ");
        }
    }

    static final class Silence implements Channel {
        public String read() {
            return "";
        }

        public void close() {
            log.append("close silence; ");
        }
    }

    static Channel open(String name) {
        if (name.equals("quiet")) {
            return new Silence();
        }
        return name.isEmpty() ? null : new Pipe(name);
    }

    static String use(String name) {
        try (Channel c = open(name)) {
            return c == null ? "nothing" : c.read();
        } catch (IllegalStateException e) {
            return "failed: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(use("first") + " | " + log);
        log.setLength(0);
        System.out.println(use("") + " | " + log);
        log.setLength(0);
        System.out.println(use("bad") + " | " + log);
        log.setLength(0);
        System.out.println("[" + use("quiet") + "] | " + log);
    }
}
