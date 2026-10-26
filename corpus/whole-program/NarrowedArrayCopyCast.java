// An Object[] cast to String[] where a null test has narrowed it, as DiagnosticCommandImpl.invoke
// prints its signature: the copy is not null.
import java.util.Arrays;
public class NarrowedArrayCopyCast {
    static String describe(String action, Object[] params, Object[] signature) {
        if (params != null && signature != null && params.length != signature.length) {
            return action + ": mismatched signature "
                + (signature != null ? Arrays.toString((String[]) signature) : "[]")
                + " or parameters";
        }
        return action + " ok";
    }
    public static void main(String[] args) {
        System.out.println(describe("a", new Object[1], new String[] {"int", "long"}));
        System.out.println(describe("b", null, null));
        System.out.println(describe("c", new Object[2], new String[] {"x", null}));
    }
}
