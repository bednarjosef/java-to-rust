import java.util.Iterator;
import java.util.NoSuchElementException;

public class ForEachIterable {
    // A user list whose iterator is an inner class.
    static class Names implements Iterable<String> {
        private String[] items = new String[4];
        private int size;

        void add(String s) {
            if (size == items.length) {
                String[] bigger = new String[size * 2];
                for (int i = 0; i < size; i++) bigger[i] = items[i];
                items = bigger;
            }
            items[size++] = s;
        }

        int size() { return size; }

        public Iterator<String> iterator() { return new Cursor(); }

        class Cursor implements Iterator<String> {
            private int next;
            public boolean hasNext() { return next < size; }
            public String next() {
                if (next >= size) throw new NoSuchElementException();
                return items[next++];
            }
        }
    }

    // A range whose iterator is an anonymous class, over boxed integers.
    static class Range implements Iterable<Integer> {
        final int from;
        final int to;
        Range(int from, int to) { this.from = from; this.to = to; }
        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                int at = from;
                public boolean hasNext() { return at < to; }
                public Integer next() { return at++; }
            };
        }
    }

    public static void main(String[] args) {
        Names names = new Names();
        for (String s : new String[] { "ada", "bo", "cy", "dee", "ed" }) names.add(s);
        StringBuilder all = new StringBuilder();
        for (String s : names) all.append(s).append(' ');
        System.out.println(all.toString().trim());

        int sum = 0;
        for (int i : new Range(1, 5)) sum += i;
        System.out.println("sum " + sum);

        // Nested loops over two iterables, with break and continue.
        StringBuilder pairs = new StringBuilder();
        outer:
        for (String s : names) {
            for (Integer k : new Range(0, 3)) {
                if (k == 1) continue;
                if (s.length() == 3 && k == 2) continue outer;
                pairs.append(s).append(k).append(',');
            }
            if (s.equals("dee")) break;
        }
        System.out.println(pairs);

        // The body changes the collection: the loop sees the new size, as Java's does.
        for (String s : names) {
            if (names.size() < 7) names.add(s + "!");
        }
        System.out.println(names.size());
        Iterable<String> it = names;
        int count = 0;
        for (String s : it) count += s.length();
        System.out.println("letters " + count);
    }
}
