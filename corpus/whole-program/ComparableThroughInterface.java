import java.util.*;

// A class that is comparable only through an interface it implements, as the JDK's
// `ScheduledThreadPoolExecutor.ScheduledFutureTask` is through `Delayed extends
// Comparable<Delayed>`, compared as a raw `Comparable` by a `TreeSet` and a sort, next to an enum.
public class ComparableThroughInterface {
    interface Delayed extends Comparable<Delayed> {
        long delay();
    }

    static final class Task implements Delayed {
        final long d;
        Task(long d) { this.d = d; }
        public long delay() { return d; }
        public int compareTo(Delayed other) { return Long.compare(d, other.delay()); }
        public String toString() { return "task " + d; }
    }

    enum Color { RED, GREEN }

    public static void main(String[] args) {
        TreeSet<Object> set = new TreeSet<>();
        set.add(new Task(3));
        set.add(new Task(1));
        System.out.println(set);
        TreeSet<Object> colors = new TreeSet<>(List.of(Color.GREEN, Color.RED));
        System.out.println(colors);
        List<Comparable> raw = new ArrayList<>(List.of(new Task(5), new Task(2)));
        Collections.sort(raw);
        System.out.println(raw);
    }
}
