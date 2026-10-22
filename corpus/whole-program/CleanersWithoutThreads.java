import java.lang.ref.Cleaner;

/** A cleaner runs an action when `clean()` is called; its thread would only wait for collections. */
public class CleanersWithoutThreads {
    static class Resource {
        final String name;

        Resource(String name) {
            this.name = name;
        }
    }

    public static void main(String[] args) {
        Cleaner cleaner = Cleaner.create();
        Resource resource = new Resource("first");
        String name = resource.name;
        Cleaner.Cleanable cleanable = cleaner.register(resource, () -> System.out.println("cleaned " + name));
        System.out.println("registered " + resource.name);
        cleanable.clean();
        // A second clean does nothing.
        cleanable.clean();
        Cleaner other = Cleaner.create(r -> new Thread(r, "named cleaner"));
        Cleaner.Cleanable second = other.register(new Resource("second"), () -> System.out.println("second cleaned"));
        second.clean();
        System.out.println("done");
    }
}
