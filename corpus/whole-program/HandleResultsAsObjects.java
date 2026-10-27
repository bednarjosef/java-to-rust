import java.lang.invoke.MethodHandle;

// The result of invoking a handle that no code gives a value, held as an Object, as UpcallLinker's
// invokeInterpBindings holds its leaf's result: here the handle would come from a call that always
// throws. The translation makes the call's value a handle's token, which is an Object as Java's
// result is.
public class HandleResultsAsObjects {
    static MethodHandle makeLeaf() {
        throw new IllegalStateException("no leaf");
    }

    static Object call(MethodHandle leaf, Object[] args) throws Throwable {
        Object o = leaf.invoke(args);
        return o;
    }

    public static void main(String[] args) throws Throwable {
        try {
            System.out.println(call(makeLeaf(), new Object[0]));
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
    }
}
