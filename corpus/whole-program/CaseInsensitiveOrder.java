// `String.CASE_INSENSITIVE_ORDER`: sorting, a sorted map and set keyed by it, a binary search,
// its reversal and combination with other comparators, one object at every read, and the
// exceptions it throws for null.
import java.util.*;

public class CaseInsensitiveOrder {
    record Module(String name, int size) {}

    public static void main(String[] args) {
        List<String> words = new ArrayList<>(List.of("beta", "Alpha", "gamma", "ALPHA", "Beta", "délta", "Delta", "ß"));
        words.sort(String.CASE_INSENSITIVE_ORDER);
        System.out.println(words);
        words.sort(String.CASE_INSENSITIVE_ORDER.reversed());
        System.out.println(words);

        TreeMap<String, Integer> headers = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
        headers.put("Content-Type", 1);
        headers.put("content-type", 2);
        headers.put("Accept", 3);
        System.out.println(headers + " " + headers.get("CONTENT-TYPE") + " " + headers.containsKey("accept"));

        Set<String> hosts = new TreeSet<>(String.CASE_INSENSITIVE_ORDER);
        Collections.addAll(hosts, "Example.com", "example.COM", "other.org");
        System.out.println(hosts + " " + hosts.contains("EXAMPLE.com"));

        String[] names = { "b", "A", "c", "D" };
        Arrays.sort(names, String.CASE_INSENSITIVE_ORDER);
        System.out.println(Arrays.toString(names) + " " + Arrays.binarySearch(names, "C", String.CASE_INSENSITIVE_ORDER));

        List<Module> modules = new ArrayList<>(List.of(new Module("zlib", 3), new Module("Netty", 9), new Module("ASM", 2), new Module("netty", 1)));
        modules.sort(Comparator.comparing(Module::name, String.CASE_INSENSITIVE_ORDER).thenComparingInt(Module::size));
        System.out.println(modules);

        Comparator<String> order = String.CASE_INSENSITIVE_ORDER;
        System.out.println((order == String.CASE_INSENSITIVE_ORDER) + " " + Integer.signum(order.compare("abc", "ABD")) + " " + order.compare("x", "X"));
        System.out.println(order.compare("İ", "i") + " " + "İ".compareToIgnoreCase("i"));
        try {
            order.compare(null, "a");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            order.compare("a", null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
