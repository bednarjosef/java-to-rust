import java.lang.reflect.Modifier;
import java.util.function.Supplier;

// `Class.getModifiers()` and where a class is declared (`isMemberClass`, `isLocalClass`,
// `isAnonymousClass`, `getEnclosingClass`, `getDeclaringClass`), as gson asks of every class it
// adapts: classes of every kind and nesting, arrays and primitive types.
public class ClassModifiersAndNesting {
    public static class PublicStatic {}

    private class PrivateInner {}

    protected abstract static class ProtectedAbstract {}

    static final class FinalMember {}

    interface Shape {
        class Nested {}
    }

    enum Plain { A, B }

    enum WithBodies {
        A {
            @Override
            int size() { return 1; }
        };

        abstract int size();
    }

    record Point(int x, int y) {}

    @FunctionalInterface
    public interface Action {
        void run();
    }

    static void show(String label, Class<?> c) {
        Class<?> enclosing = c.getEnclosingClass();
        Class<?> declaring = c.getDeclaringClass();
        System.out.println(label + ": " + c.getModifiers() + " [" + Modifier.toString(c.getModifiers()) + "]"
                + " member " + c.isMemberClass() + " local " + c.isLocalClass()
                + " anonymous " + c.isAnonymousClass()
                + " enclosing " + (enclosing == null ? "null" : enclosing.getSimpleName())
                + " declaring " + (declaring == null ? "null" : declaring.getSimpleName()));
    }

    public static void main(String[] args) {
        class Local {}
        Runnable anonymous = new Runnable() {
            public void run() {}
        };
        Supplier<Object> inStatic = () -> new Object() {};
        show("top", ClassModifiersAndNesting.class);
        show("public static", PublicStatic.class);
        show("private inner", PrivateInner.class);
        show("protected abstract", ProtectedAbstract.class);
        show("final member", FinalMember.class);
        show("interface", Shape.class);
        show("nested in interface", Shape.Nested.class);
        show("enum", Plain.class);
        show("enum with bodies", WithBodies.class);
        show("record", Point.class);
        show("annotated interface", Action.class);
        show("local", Local.class);
        show("anonymous", anonymous.getClass());
        show("anonymous in lambda", inStatic.get().getClass());
        show("array", PublicStatic[].class);
        show("private array", PrivateInner[][].class);
        show("int", int.class);
        show("string", String.class);
        show("object", Object.class);
        show("integer", Integer.class);
        show("number", Number.class);
        System.out.println(new PublicStatic().getClass().getModifiers() == PublicStatic.class.getModifiers());
    }
}
