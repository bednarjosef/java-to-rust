import java.util.AbstractCollection;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

public class NullIterators {
    static class Bag extends AbstractCollection<String> {
        final boolean empty;

        Bag(boolean empty) {
            this.empty = empty;
        }

        public Iterator<String> iterator() {
            return empty ? null : List.of("a", "b").iterator();
        }

        public int size() {
            return 2;
        }
    }

    static int count(Bag bag) {
        int n = 0;
        for (String s : bag) {
            n += s.length();
        }
        return n;
    }

    // The collection is a call whose result javac casts from `Object`.
    static int total(Map<String, Bag> bags, String key) {
        int n = 0;
        for (String s : bags.get(key)) {
            n += s.length();
        }
        return n;
    }

    public static void main(String[] args) {
        System.out.println(count(new Bag(false)));
        try {
            System.out.println(count(new Bag(true)));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(count(null));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        Map<String, Bag> bags = Map.of("full", new Bag(false), "empty", new Bag(true));
        System.out.println(total(bags, "full"));
        try {
            System.out.println(total(bags, "empty"));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(total(bags, "none"));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        // The JDK's for-each loops: AbstractCollection.addAll's.
        HashSet<String> set = new HashSet<>();
        System.out.println(set.addAll(new Bag(false)) + " " + set.size());
        try {
            System.out.println(new HashSet<String>().addAll(new Bag(true)));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(new HashSet<String>().addAll(null));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
