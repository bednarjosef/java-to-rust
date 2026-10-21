// The standard exceptions of `java.lang` for reflection and security: the `ReflectiveOperationException`
// family (with `ClassNotFoundException.getException`) and the four constructors of `SecurityException`,
// thrown and caught as themselves and as their superclasses, with messages, causes and `toString`.
public class ReflectiveStandardExceptions {
    static Class<?> find(String name) throws ReflectiveOperationException {
        switch (name) {
            case "string":
                return String.class;
            case "missing":
                throw new ClassNotFoundException("no class " + name);
            case "wrapped":
                throw new ClassNotFoundException("no class " + name, new IllegalStateException("loader"));
            case "method":
                throw new ReflectiveOperationException(new NoSuchMethodException("run"));
            default:
                throw new ClassNotFoundException();
        }
    }

    static void member(String kind) throws ReflectiveOperationException {
        if (kind.equals("method")) {
            throw new NoSuchMethodException("run()");
        }
        if (kind.equals("field")) {
            throw new NoSuchFieldException("size");
        }
        if (kind.equals("access")) {
            throw new IllegalAccessException("private member");
        }
        throw new InstantiationException("abstract class");
    }

    static void guard(String shape) {
        switch (shape) {
            case "none":
                throw new SecurityException();
            case "message":
                throw new SecurityException("denied");
            case "both":
                throw new SecurityException("denied", new RuntimeException("policy"));
            default:
                throw new SecurityException(new IllegalStateException("policy"));
        }
    }

    public static void main(String[] args) {
        for (String name : new String[] {"string", "missing", "wrapped", "method", "other"}) {
            try {
                System.out.println("found " + find(name).getName());
            } catch (ClassNotFoundException e) {
                System.out.println(e.getMessage());
                System.out.println(e.getCause());
                System.out.println(e.getException() == e.getCause());
                System.out.println(e);
            } catch (ReflectiveOperationException e) {
                System.out.println("reflective: " + e);
                System.out.println(e.getCause().getMessage());
            }
        }

        for (String kind : new String[] {"method", "field", "access", "new"}) {
            try {
                member(kind);
            } catch (NoSuchMethodException | NoSuchFieldException e) {
                System.out.println("no member: " + e.getMessage() + " / " + e);
            } catch (ReflectiveOperationException e) {
                System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
                System.out.println(e.getCause() == null);
            }
        }

        ClassNotFoundException bare = new ClassNotFoundException("direct", null);
        System.out.println(bare.getException() + " " + bare);
        ReflectiveOperationException superclass = new InstantiationException();
        System.out.println(superclass.getMessage() + " / " + superclass);
        Exception asException = new NoSuchFieldException("x");
        System.out.println(asException.getMessage());

        for (String shape : new String[] {"none", "message", "both", "cause"}) {
            try {
                guard(shape);
            } catch (SecurityException e) {
                System.out.println(e.getMessage());
                System.out.println(e.getCause());
                System.out.println(e);
            }
        }

        try {
            guard("cause");
        } catch (RuntimeException e) {
            System.out.println("as RuntimeException: " + e.getClass().getName());
        }
    }
}
