import static java.lang.Character.MAX_RADIX;
import static java.lang.Double.PRECISION;
import static java.lang.invoke.MethodHandleInfo.REF_invokeInterface;

// Compile-time constants of library classes that a static import names, which are their values.
public class StaticImportedConstants {
    public static void main(String[] args) {
        System.out.println(REF_invokeInterface + " " + PRECISION + " " + MAX_RADIX);
        int[] table = new int[REF_invokeInterface + 1];
        table[REF_invokeInterface] = PRECISION;
        System.out.println(table.length + " " + table[9]);
    }
}
