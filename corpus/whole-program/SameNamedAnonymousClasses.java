import java.util.Map;
import java.util.stream.Collectors;
import java.util.stream.Stream;

// Two anonymous classes of the JDK with one name in two packages: the `AbstractSet` that
// `entrySet()` of `Map.of` makes in `java.util`, and the one of `partitioningBy`'s map in
// `java.util.stream`. Both are variants of the enums of `Set` and `Object`.
public class SameNamedAnonymousClasses {
    public static void main(String[] args) {
        Map<String, Integer> sizes = Map.of("ant", 3, "bee", 3, "wasp", 4);
        int total = 0;
        for (Map.Entry<String, Integer> e : sizes.entrySet()) {
            total += e.getValue();
        }
        System.out.println(total + " " + sizes.entrySet().size());
        Map<Boolean, Long> parts =
                Stream.of(1, 2, 3, 4, 5).collect(Collectors.partitioningBy(x -> x > 2, Collectors.counting()));
        Object shown = parts.entrySet();
        System.out.println(shown + " " + parts.get(true));
    }
}
