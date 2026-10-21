public class AddSuppressedChecks {
    public static void main(String[] args) {
        Exception primary = new Exception("primary");
        try {
            primary.addSuppressed(primary);
        } catch (IllegalArgumentException e) {
            System.out.println("self: " + e.getMessage());
        }
        try {
            primary.addSuppressed(null);
        } catch (NullPointerException e) {
            System.out.println("literal null: " + e.getMessage());
        }
        Exception missing = args.length > 100 ? new Exception("unused") : null;
        try {
            primary.addSuppressed(missing);
        } catch (NullPointerException e) {
            System.out.println("variable null: " + e.getMessage());
        }
        System.out.println("still suppressed: " + primary.getSuppressed().length);

        Exception other = new Exception("other");
        primary.addSuppressed(other);
        System.out.println("suppressed: " + primary.getSuppressed().length + " " + primary.getSuppressed()[0].getMessage());
        Exception twin = new Exception("primary");
        primary.addSuppressed(twin);
        System.out.println("twin: " + primary.getSuppressed().length);

        Throwable t = new Throwable("t");
        try {
            t.addSuppressed(t);
        } catch (Throwable e) {
            System.out.println("as Throwable: " + e.getMessage());
        }
        RuntimeException r = new RuntimeException("r");
        try {
            r.addSuppressed(r);
        } catch (IllegalArgumentException e) {
            System.out.println("runtime self: " + (e instanceof RuntimeException) + " " + e.getMessage());
        }
        System.out.println("done: " + r.getSuppressed().length);
    }
}
