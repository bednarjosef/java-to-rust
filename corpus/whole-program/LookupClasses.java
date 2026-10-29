import java.lang.invoke.MethodHandles;

/**
 * `lookupClass()` of a lookup that lowering follows is the class it looks up in; of one it does
 * not follow (here a hidden class defined from bytes, which a translated program cannot load),
 * the code sees a lookup that fails, which it catches.
 */
public class LookupClasses {
    static Class<?> defined(byte[] bytes) {
        try {
            MethodHandles.Lookup lookup = MethodHandles.lookup().defineHiddenClass(bytes, true);
            return lookup.lookupClass();
        } catch (IllegalAccessException | UnsupportedOperationException | ClassFormatError e) {
            return null;
        }
    }

    public static void main(String[] args) {
        MethodHandles.Lookup lookup = MethodHandles.lookup();
        System.out.println(lookup.lookupClass().getName());
        System.out.println(MethodHandles.lookup().lookupClass() == LookupClasses.class);
        System.out.println(defined(new byte[] {1, 2, 3}));
    }
}
