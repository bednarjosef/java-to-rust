// try-with-resources over existing fields, `try (this.owned)`, `try (shared)` and a static final
// field: each is closed as the variable holds it when the statement starts, which the body cannot
// change for a final field, and a field that is null is not closed.
public class TryWithExistingField {
    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
            System.out.println("open " + name);
        }

        String read(int i) {
            return name + "[" + i + "]";
        }

        @Override
        public void close() {
            System.out.println("close " + name);
        }
    }

    static final Resource SHARED = new Resource("static");

    final Resource owned;
    final Resource missing;

    TryWithExistingField(String name) {
        this.owned = new Resource(name);
        this.missing = null;
    }

    String run(int i) {
        StringBuilder out = new StringBuilder();
        try (this.owned) {
            out.append(owned.read(i));
        }
        try (owned) {
            out.append(owned.read(i + 1));
        } catch (IllegalStateException e) {
            out.append("unused");
        }
        try (missing) {
            out.append("|missing");
        }
        try (SHARED) {
            out.append("|").append(SHARED.read(i + 2));
        } finally {
            System.out.println("finally " + i);
        }
        return out.toString();
    }

    public static void main(String[] args) {
        TryWithExistingField first = new TryWithExistingField("first");
        System.out.println(first.run(0));
        TryWithExistingField second = new TryWithExistingField("second");
        System.out.println(second.run(5));
        try (second.owned) {
            System.out.println("main reads " + second.owned.read(9));
            throw new IllegalStateException("body fails");
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
