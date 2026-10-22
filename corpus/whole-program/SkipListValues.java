import java.util.concurrent.ConcurrentSkipListMap;
import java.util.concurrent.ConcurrentSkipListSet;

// A concurrent skip list whose values are strings and boxes, which its compare-and-set of a
// node's value compares with the value it has just read: a set (whose values are all
// `Boolean.TRUE`) and a map whose values are replaced, removed and merged.
public class SkipListValues {
    public static void main(String[] args) {
        ConcurrentSkipListSet<String> set = new ConcurrentSkipListSet<>();
        for (String s : new String[] {"pear", "apple", "fig", "apple"}) {
            System.out.println("add " + s + " " + set.add(s));
        }
        System.out.println(set + " remove fig " + set.remove("fig") + " again " + set.remove("fig"));
        ConcurrentSkipListMap<String, Integer> map = new ConcurrentSkipListMap<>();
        map.put("b", 2);
        map.put("a", 1000);
        System.out.println(map.put("b", 20) + " " + map.putIfAbsent("b", 3) + " " + map);
        System.out.println(map.replace("a", 1000, 1) + " " + map.replace("a", 5, 6) + " " + map);
        System.out.println(map.remove("b", 21) + " " + map.remove("b", 20) + " " + map);
        map.merge("a", 10, Integer::sum);
        map.compute("c", (k, v) -> v == null ? 7 : v + 1);
        map.computeIfPresent("c", (k, v) -> v * 2);
        System.out.println(map + " " + map.firstKey() + " " + map.lastEntry());
    }
}
