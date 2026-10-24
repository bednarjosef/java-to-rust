// A class that overrides `finalize` and calls `super.finalize()` of `Object`, which does
// nothing; the program calls its `finalize` itself, as no collector is relied on to.
public class SuperFinalize {
    static int released;

    static class Resource {
        final String name;

        Resource(String name) { this.name = name; }

        @Override
        protected void finalize() throws Throwable {
            try {
                released++;
                System.out.println("released " + name);
            } finally {
                super.finalize();
            }
        }
    }

    static class Pool extends Resource {
        Pool() { super("pool"); }

        @Override
        protected void finalize() throws Throwable {
            System.out.println("pool first");
            super.finalize();
        }
    }

    public static void main(String[] args) throws Throwable {
        new Resource("a").finalize();
        new Pool().finalize();
        System.out.println("released " + released);
    }
}
