// A method reference bound to a lambda's parameter whose type is inferred as a capture of `?`
// (Minecraft's `codec().map(v -> v::matches)` over a `Type<?>`): the parameter has the type of
// the capture's bound, as the reference's receiver does.
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import java.util.function.Predicate;

public class MethodReferenceOnCapturedParameter {
    interface Pred {
        boolean matches(String s);
    }

    record Starts(String prefix) implements Pred {
        public boolean matches(String s) {
            return s.startsWith(prefix);
        }
    }

    interface Kind<T extends Pred> {
        List<T> values();
    }

    record StartsKind(List<Starts> values) implements Kind<Starts> {}

    static <A, B> List<B> map(List<A> list, Function<? super A, ? extends B> f) {
        List<B> out = new ArrayList<>();
        for (A a : list) {
            out.add(f.apply(a));
        }
        return out;
    }

    static List<Predicate<String>> predicates(Kind<?> kind) {
        return map(kind.values(), v -> v::matches);
    }

    public static void main(String[] args) {
        Kind<?> kind = new StartsKind(List.of(new Starts("a"), new Starts("b")));
        for (Predicate<String> p : predicates(kind)) {
            System.out.println(p.test("apple") + " " + p.test("banana"));
        }
    }
}
