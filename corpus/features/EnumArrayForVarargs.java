// An array of an enum passed for the varargs of a type variable bounded by Enum, as javac's
// Infer passes `rest` to `EnumSet.of(ib, rest)`.
public class EnumArrayForVarargs {
    enum Bound {
        EQ,
        LOWER,
        UPPER
    }

    @SafeVarargs
    static <E extends Enum<E>> String of(E first, E... rest) {
        StringBuilder sb = new StringBuilder(first.name());
        for (E e : rest) {
            sb.append(",").append(e.name());
        }
        return sb.toString();
    }

    static String check(Bound ib, Bound... rest) {
        return of(ib, rest);
    }

    public static void main(String[] args) {
        System.out.println(check(Bound.EQ, Bound.LOWER, Bound.UPPER));
        System.out.println(check(Bound.UPPER));
        System.out.println(of(Bound.LOWER, Bound.EQ));
    }
}
