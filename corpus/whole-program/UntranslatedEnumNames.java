import java.lang.invoke.VarHandle;
import java.util.function.Function;
import java.util.function.ToIntFunction;

// The name and ordinal of a constant of an enum that the translation leaves out
// (`java.lang.invoke` is not translated), in code that only ever sees null: a method reference
// to `name` that is created but never applied, and a null that throws.
public class UntranslatedEnumNames {
    static VarHandle.AccessMode mode(String[] args) {
        return null;
    }

    public static void main(String[] args) {
        Function<VarHandle.AccessMode, String> name = VarHandle.AccessMode::name;
        ToIntFunction<VarHandle.AccessMode> ordinal = VarHandle.AccessMode::ordinal;
        VarHandle.AccessMode m = mode(args);
        if (args.length > 5) {
            System.out.println(name.apply(m) + ordinal.applyAsInt(m));
        }
        try {
            System.out.println(m.name());
        } catch (NullPointerException e) {
            System.out.println("null mode");
        }
        System.out.println(name != null && ordinal != null);
    }
}
