// The JDK's collections that compare their elements by identity, holding strings and boxes:
// `WeakHashMap` matches a key by `equals` after identity, `CopyOnWriteArrayList.set` and
// `remove` skip work on the same element, and an `IdentityHashMap` of the program's objects is
// asked about strings, which are never one of its keys.
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.concurrent.CopyOnWriteArrayList;

public class ValueIdentityCollections {
    static final class Node {
        final String name;

        Node(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    public static void main(String[] args) {
        Map<String, Integer> weak = new WeakHashMap<>();
        String key = "k" + args.length;
        weak.put(key, 1000);
        weak.put(new StringBuilder("k").append(0).toString(), 1000);
        weak.put("other", 2000);
        System.out.println(weak.size() + " " + weak.get("k0") + " " + weak.containsKey("other"));

        List<Object> list = new CopyOnWriteArrayList<>();
        list.add("a");
        list.add(1000);
        list.add("b");
        System.out.println(list.set(1, 1000) + " " + list.set(0, "a" + "") + " " + list);
        System.out.println(list.remove("b") + " " + list.remove(Integer.valueOf(1000)) + " " + list);

        Map<Object, Object> identity = new IdentityHashMap<>();
        Node first = new Node("first");
        Node second = new Node("second");
        identity.put(first, "one");
        identity.put(second, "two");
        System.out.println(identity.get(first) + " " + identity.get("first") + " " + identity.containsKey(second));
        Set<Object> seen = Collections.newSetFromMap(new IdentityHashMap<>());
        System.out.println(seen.add(first) + " " + seen.add(first) + " " + seen.contains("first") + " " + seen.size());
    }
}
