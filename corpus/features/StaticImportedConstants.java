import static java.lang.Boolean.FALSE;
import static java.lang.Boolean.TRUE;
import static java.lang.Character.MAX_RADIX;
import static java.lang.Double.PRECISION;
import static java.lang.invoke.MethodHandleInfo.REF_invokeInterface;

// Compile-time constants of library classes that a static import names, which are their values,
// and `Boolean`'s `TRUE` and `FALSE`, which are its cached boxes.
public class StaticImportedConstants {
    static boolean isTrue(Object value) {
        return "true".equals(value) || TRUE.equals(value);
    }

    public static void main(String[] args) {
        System.out.println(REF_invokeInterface + " " + PRECISION + " " + MAX_RADIX);
        int[] table = new int[REF_invokeInterface + 1];
        table[REF_invokeInterface] = PRECISION;
        System.out.println(table.length + " " + table[9]);
        System.out.println(isTrue(TRUE) + " " + isTrue("true") + " " + isTrue(FALSE) + " " + isTrue(null));
        Boolean box = TRUE;
        System.out.println((box == Boolean.TRUE) + " " + (FALSE == Boolean.valueOf(false)));
    }
}
