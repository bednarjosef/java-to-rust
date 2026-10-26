import java.util.List;
import java.util.Locale;
import java.util.function.Function;

// Lambdas, method references and captured locals among the arguments of `String.format` with a
// locale, as the Minecraft server's `ServerLevel.getWatchdogStats` passes them.
public class LambdaInFormat {
    record Thing(String name) {}

    private static <T> String count(Iterable<T> values, Function<T, String> getter) {
        StringBuilder b = new StringBuilder();
        for (T v : values) {
            b.append(getter.apply(v)).append(';');
        }
        return b.toString();
    }

    final List<Thing> things = List.of(new Thing("a"), new Thing("b"));

    String stats(String suffix) {
        return String.format(
            Locale.ROOT,
            "things: %s [%s], again %d [%s] %s",
            this.things.size(),
            count(this.things, t -> t.name() + suffix),
            2,
            count(this.things, Thing::name),
            new Object() {
                @Override
                public String toString() {
                    return "anonymous " + suffix;
                }
            }
        );
    }

    public static void main(String[] args) {
        System.out.println(new LambdaInFormat().stats("!"));
    }
}
