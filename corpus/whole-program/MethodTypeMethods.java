import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;

// Methods of `MethodType` besides `methodType`, as `HBShaper` of `java.desktop` builds the type
// of an upcall: the translation does not follow them, and they throw where they run, which this
// program does not reach.
public class MethodTypeMethods {
    static int bump(Object bound, int a) {
        return a + 1;
    }

    static MethodHandle bound(Class<?> clazz, Object bindArg, String name) {
        try {
            MethodType type = MethodType.methodType(int.class, int.class);
            type = type.insertParameterTypes(0, clazz);
            System.out.println(type.parameterCount());
            MethodHandle mh = MethodHandles.lookup().findStatic(MethodTypeMethods.class, name, type);
            return mh.bindTo(bindArg);
        } catch (IllegalAccessException | NoSuchMethodException e) {
            return null;
        }
    }

    public static void main(String[] args) {
        if (args.length > 5) {
            System.out.println(bound(Object.class, "x", "bump") == null);
        }
        System.out.println("built");
    }
}
