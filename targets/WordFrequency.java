// Features: HashMap and TreeMap with String keys and Integer values, merge, getOrDefault,
// entrySet and for-each loops, String.split, text blocks, printf, Map.Entry, StringBuilder.
import java.util.HashMap;
import java.util.Map;
import java.util.TreeMap;

public class WordFrequency {
    static final String TEXT = """
            The quick brown fox jumps over the lazy dog. The dog sleeps;
            the fox runs away! A quick fox is a happy fox, and the dog is not.
            Words repeat: fox, dog, the, the, THE.
            """;

    static String normalize(String word) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < word.length(); i++) {
            char c = word.charAt(i);
            if (Character.isLetter(c)) {
                sb.append(Character.toLowerCase(c));
            }
        }
        return sb.toString();
    }

    static Map<String, Integer> count(String text) {
        Map<String, Integer> counts = new HashMap<>();
        for (String raw : text.split("\\s+")) {
            String word = normalize(raw);
            if (word.isEmpty()) {
                continue;
            }
            counts.merge(word, 1, Integer::sum);
        }
        return counts;
    }

    static String bar(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < n; i++) {
            sb.append('#');
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        Map<String, Integer> counts = count(TEXT);
        System.out.println("distinct words: " + counts.size());

        TreeMap<String, Integer> sorted = new TreeMap<>(counts);
        System.out.println("alphabetical:");
        for (Map.Entry<String, Integer> e : sorted.entrySet()) {
            System.out.printf("  %-6s %2d %s%n", e.getKey(), e.getValue(), bar(e.getValue()));
        }

        String top = null;
        int topCount = 0;
        for (Map.Entry<String, Integer> e : counts.entrySet()) {
            if (e.getValue() > topCount || (e.getValue() == topCount && e.getKey().compareTo(top) < 0)) {
                top = e.getKey();
                topCount = e.getValue();
            }
        }
        System.out.println("most common: " + top + " (" + topCount + ")");

        System.out.println("missing 'cat': " + counts.getOrDefault("cat", 0));
        System.out.println("first key of tree: " + sorted.firstKey());
        System.out.println("last key of tree: " + sorted.lastKey());
        System.out.println("headMap(\"dog\"): " + sorted.headMap("dog"));

        int total = 0;
        for (int v : counts.values()) {
            total += v;
        }
        System.out.println("total words: " + total);

        Map<Integer, Integer> byLength = new TreeMap<>();
        for (String word : sorted.keySet()) {
            byLength.merge(word.length(), 1, Integer::sum);
        }
        System.out.println("by length: " + byLength);
        counts.remove("the");
        System.out.println("after removing 'the': " + counts.containsKey("the") + " " + counts.size());
    }
}
