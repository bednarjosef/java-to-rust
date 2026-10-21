// Views of a TreeMap between bounds: their iterators stop at the fence entry by comparing keys
// with `==` (TreeMap.NavigableSubMap.SubMapIterator), which the translation compares by value.
// The keys of one sorted map are distinct under its comparator, so that gives Java's result, also
// for bounds that are equal to a key but other objects, and for boxes outside the cached range.
import java.util.Map;
import java.util.TreeMap;

public class SortedSubMaps {
    public static void main(String[] args) {
        TreeMap<String, Integer> words = new TreeMap<>();
        for (String w : "the quick brown fox jumps over the lazy dog".split(" ")) {
            words.merge(w, 1, Integer::sum);
        }
        String dog = new String(new char[] {'d', 'o', 'g'});
        System.out.println(words.headMap(dog));
        System.out.println(words.headMap(dog, true));
        System.out.println(words.tailMap("over"));
        System.out.println(words.subMap("brown", "lazy"));
        System.out.println(words.subMap("c", "p").keySet());
        for (Map.Entry<String, Integer> e : words.tailMap("quick", false).entrySet()) {
            System.out.println(e.getKey() + "=" + e.getValue());
        }

        TreeMap<Integer, String> numbers = new TreeMap<>();
        for (int i = 0; i < 10; i++) {
            numbers.put(1000 + i * 100, "n" + i);
        }
        System.out.println(numbers.headMap(1500));
        System.out.println(numbers.subMap(1200, true, 1700, true).values());
        int total = 0;
        for (int k : numbers.tailMap(1800).keySet()) {
            total += k;
        }
        System.out.println(total);
    }
}
