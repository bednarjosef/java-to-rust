import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/** Object's equals and hashCode, through Object, for classes that keep them: identity. */
public class IdentityThroughObject {
    static class Empty {}

    static class Counter {
        int count;
    }

    static final class Named extends Counter {
        final String name;

        Named(String name) {
            this.name = name;
        }
    }

    record Point(int x, int y) {}

    public static void main(String[] args) {
        List<Object> objects = new ArrayList<>();
        objects.add(new Empty());
        objects.add(new Empty());
        Counter counter = new Counter();
        counter.count = 3;
        objects.add(counter);
        objects.add(new Named("n"));
        objects.add(new Point(1, 2));
        objects.add(new Point(1, 2));
        objects.add("text");
        objects.add(7);
        for (int i = 0; i < objects.size(); i++) {
            StringBuilder row = new StringBuilder();
            for (Object other : objects) {
                row.append(objects.get(i).equals(other) ? '1' : '0');
            }
            Object o = objects.get(i);
            boolean identity = !(o instanceof Point || o instanceof String || o instanceof Integer);
            row.append(' ').append(o.equals(null));
            if (identity) {
                row.append(' ').append(o.hashCode() == System.identityHashCode(o));
            }
            System.out.println(row);
        }
        Map<Object, Integer> positions = new HashMap<>();
        Map<Object, Integer> identities = new IdentityHashMap<>();
        for (int i = 0; i < objects.size(); i++) {
            positions.put(objects.get(i), i);
            if (i < 4) {
                identities.put(objects.get(i), i);
            }
        }
        for (Object o : objects) {
            System.out.println(positions.get(o) + " " + identities.get(o));
        }
        counter.count++;
        System.out.println(positions.get(counter) + " " + (objects.get(0) == objects.get(1)));
    }
}
