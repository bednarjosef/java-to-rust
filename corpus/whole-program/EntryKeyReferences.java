import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import java.util.stream.Collectors;

// `Map.Entry::getKey` and `Map.Entry::getValue` as functions of a stream of entries, as
// `CompactNumberFormat` and the CLDR locale provider use them: the key is the map's key type,
// though the entry's method gives an erased `Object`.
public class EntryKeyReferences {
    public static void main(String[] args) {
        Map<String, Integer> counts = new TreeMap<>(Map.of("one", 1, "two", 2, "three", 3));
        System.out.println(counts.entrySet().stream()
                .filter(e -> e.getValue() > 1)
                .map(Map.Entry::getKey)
                .collect(Collectors.joining(",")));
        Optional<String> first = counts.entrySet().stream().map(Map.Entry::getKey).findFirst();
        System.out.println(first.orElse("none") + " " + first.map(String::length).orElse(0));
        System.out.println(counts.entrySet().stream().map(Map.Entry::getValue).mapToInt(Integer::intValue).sum());
    }
}
