import java.util.List;

// Record patterns of generic records, as the Minecraft server's `CubicSpline.codec` takes a
// spline apart with `case CubicSpline.Constant(float value)`.
public class GenericRecordPatterns {
    interface Spline<I> {}

    record Constant<I>(float value) implements Spline<I> {}

    record Multipoint<I extends CharSequence>(I coordinate, float[] locations, List<Spline<I>> values) implements Spline<I> {}

    record Pair<A, B>(A first, B second) {}

    static <I extends CharSequence> String describe(Spline<I> spline) {
        return switch (spline) {
            case Constant(float value) -> "constant " + value;
            case Multipoint<I>(I coordinate, float[] locations, List<Spline<I>> values) ->
                "multipoint on " + coordinate + " with " + locations.length + " points, " + values.size() + " values";
            default -> throw new IllegalStateException();
        };
    }

    static String pair(Object o) {
        if (o instanceof Pair<?, ?>(String s, Integer n)) {
            return s.repeat(n);
        }
        if (o instanceof Pair<?, ?>(var a, Pair<?, ?>(var b, var c))) {
            return a + "/" + b + "/" + c;
        }
        return "other";
    }

    public static void main(String[] args) {
        System.out.println(describe(new Constant<String>(1.5f)));
        System.out.println(describe(new Multipoint<>("x", new float[] {0, 1}, List.of(new Constant<>(2f)))));
        System.out.println(pair(new Pair<>("ab", 3)));
        System.out.println(pair(new Pair<>(1, new Pair<>(2, "three"))));
        System.out.println(pair("none"));
    }
}
