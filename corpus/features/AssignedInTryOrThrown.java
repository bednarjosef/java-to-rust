// Locals declared without a value and set in a `try` with a `finally`, in an `if` whose other
// branch throws, as the JDK's TLS key derivation does, or in a `catch` of a block that returns,
// as its key unwrapping does: the code after the `try` reads them.
public class AssignedInTryOrThrown {
    interface Key {
        String name();
    }

    record Spec(String name) implements Key {}

    static int cleaned;

    static String derive(int version, boolean exportable) {
        Key client;
        Key server;
        try {
            if (!exportable) {
                client = new Spec("c" + version);
                server = new Spec("s" + version);
            } else {
                if (version >= 3) {
                    throw new IllegalStateException("no exportable suites past " + version);
                } else if (version == 2) {
                    client = new Spec("tls-c");
                    server = new Spec("tls-s");
                } else {
                    client = new Spec("ssl-c");
                    server = new Spec("ssl-s");
                }
            }
        } finally {
            cleaned++;
        }
        return client.name() + "/" + server.name();
    }

    static Key construct(String algorithm) throws Exception {
        Key key;
        try {
            if (algorithm.startsWith("sun")) {
                return new Spec("direct " + algorithm);
            }
            throw new IllegalArgumentException(algorithm);
        } catch (IllegalArgumentException e) {
            try {
                if (algorithm.isEmpty()) {
                    throw new IllegalStateException();
                }
                key = new Spec("fallback " + e.getMessage());
            } catch (IllegalStateException e2) {
                throw new Exception("none for " + algorithm);
            }
        } finally {
            cleaned++;
        }
        return key;
    }

    public static void main(String[] args) throws Exception {
        System.out.println(derive(1, false));
        System.out.println(derive(2, true));
        System.out.println(derive(1, true));
        try {
            derive(4, true);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(construct("sunRsa").name());
        System.out.println(construct("ec").name());
        try {
            construct("");
        } catch (Exception e) {
            System.out.println(e.getMessage());
        }
        System.out.println(cleaned);
    }
}
