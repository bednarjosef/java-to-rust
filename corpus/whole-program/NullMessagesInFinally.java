import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

// The messages of nulls in a `finally` block, which javac copies to each way out of its `try`:
// the copy that runs for an exception keeps the exception in a slot of its own first, so its
// variables, a for-each loop's iterator and element among them, are a slot further along.
public class NullMessagesInFinally {
    static final List<String> seen = new ArrayList<>();

    static class NoIterator implements Iterable<String> {
        public Iterator<String> iterator() {
            return null;
        }
    }

    static void drain(List<String> items, boolean fail) {
        final List<String> changed = new ArrayList<>(items);
        try {
            if (fail) {
                throw new IllegalStateException("failed");
            }
            seen.add("body");
        } finally {
            for (String c : changed) {
                seen.add(c.toUpperCase());
            }
        }
    }

    static void walk(Iterable<String> items, boolean fail) {
        int before = seen.size();
        try {
            if (fail) {
                throw new IllegalStateException("failed");
            }
        } catch (IllegalStateException e) {
            seen.add("caught " + e.getMessage());
        } finally {
            for (String s : items) {
                seen.add(s + before);
            }
        }
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (RuntimeException e) {
            System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
        }
        System.out.println(seen);
        seen.clear();
    }

    public static void main(String[] args) {
        List<String> withNull = new ArrayList<>();
        withNull.add("a");
        withNull.add(null);
        run(() -> drain(List.of("x", "y"), false));
        run(() -> drain(List.of("x", "y"), true));
        run(() -> drain(withNull, false));
        run(() -> drain(withNull, true));
        run(() -> walk(List.of("p"), false));
        run(() -> walk(List.of("p"), true));
        run(() -> walk(new NoIterator(), false));
        run(() -> walk(new NoIterator(), true));
    }
}
