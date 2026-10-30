import java.security.SecureClassLoader;

// Loaders that register as parallel capable in their static initializers, as the JDK's
// SecureClassLoader does in its own: the registration succeeds, and the loaders work.
public class ParallelCapableLoaders {
    static class Plain extends ClassLoader {
        static final boolean REGISTERED = registerAsParallelCapable();

        Plain() {
            super(ClassLoader.getSystemClassLoader());
        }
    }

    static class Secure extends SecureClassLoader {
        static final boolean REGISTERED = registerAsParallelCapable();
    }

    public static void main(String[] args) throws Exception {
        System.out.println(Plain.REGISTERED);
        System.out.println(new Plain().getParent() == ClassLoader.getSystemClassLoader());
        System.out.println(Secure.REGISTERED);
        System.out.println(new Secure().getParent() == ClassLoader.getSystemClassLoader());
    }
}
