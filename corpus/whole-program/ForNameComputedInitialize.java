// `Class.forName(name, initialize, loader)` where code computes `initialize`, as JNDI's
// `VersionHelper.loadClass` passes its own flag: the class is initialized only when it is true.
public class ForNameComputedInitialize {
    static class First {
        static {
            System.out.println("First initialized");
        }
    }

    static class Second {
        static {
            System.out.println("Second initialized");
        }

        static void touch() {
            System.out.println("Second touched");
        }
    }

    static Class<?> load(String name, boolean initialize) throws ClassNotFoundException {
        System.out.println("loading " + name + (initialize ? " and initializing" : ""));
        return Class.forName(name, initialize, ForNameComputedInitialize.class.getClassLoader());
    }

    public static void main(String[] args) throws Exception {
        Class<?> second = load("ForNameComputedInitialize$Second", false);
        System.out.println(second.getName());
        Class<?> first = load("ForNameComputedInitialize$First", true);
        System.out.println(first.getName());
        Second.touch();
        try {
            load("ForNameComputedInitialize$Missing", args.length == 0);
        } catch (ClassNotFoundException e) {
            // The message names the class and says that it is not in the translation.
            System.out.println("not found");
        }
    }
}
