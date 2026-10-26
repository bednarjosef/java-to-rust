// A method reference called through an interface whose parameters have the names the
// translation gives the reference's own (`a`, `b`, ...): Minecraft's `Blocks::never` as a
// `StateArgumentPredicate<A>`, whose `test(state, level, pos, a)` passes its `a`, cast to
// `EntityType`, as the fourth argument.
public class MethodReferenceArgumentNamedLikeItsParameters {
    interface StateArgumentPredicate<A> {
        boolean test(String state, int level, long pos, A a);
    }

    interface Swapped<A, B> {
        String join(B b, A a);
    }

    static Boolean never(String state, int level, long pos, Integer entityType) {
        return state.length() + level + pos + entityType < 0;
    }

    static String pair(String first, Integer second) {
        return first + "/" + second;
    }

    public static void main(String[] args) {
        StateArgumentPredicate<Integer> spawn = MethodReferenceArgumentNamedLikeItsParameters::never;
        System.out.println(spawn.test("stone", 2, 3L, 4) + " " + spawn.test("", -10, 1L, 2));
        Swapped<Integer, String> swapped = MethodReferenceArgumentNamedLikeItsParameters::pair;
        System.out.println(swapped.join("b", 7));
    }
}
