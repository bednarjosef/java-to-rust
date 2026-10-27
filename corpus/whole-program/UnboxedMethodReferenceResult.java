// A method reference whose method returns a type variable of a raw qualifier's class, unboxed
// for a function of a primitive (Minecraft's PlayTagWithOtherKids:
// `Comparator.comparingInt(Entry::getValue)` over a map's Integer values).
import java.util.Comparator;
import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;
import java.util.Optional;

public class UnboxedMethodReferenceResult {
    static Optional<String> leastChased(Map<String, Integer> chased) {
        return chased.entrySet()
            .stream()
            .sorted(Comparator.comparingInt(Entry::getValue))
            .filter(entry -> entry.getValue() > 0 && entry.getValue() <= 5)
            .map(Entry::getKey)
            .findFirst();
    }

    public static void main(String[] args) {
        Map<String, Integer> chased = new HashMap<>();
        chased.put("a", 4);
        chased.put("b", 2);
        chased.put("c", 0);
        chased.put("d", 9);
        System.out.println(leastChased(chased).orElse("none"));
    }
}
