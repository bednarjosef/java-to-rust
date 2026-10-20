// Features: a user Iterable<T> implemented by a generic ring buffer with a non-static inner class
// iterator that reads the outer instance's fields (Ring.this), an anonymous Iterator in a method,
// a static nested Range class implementing Iterable<Integer>, a for-each over user Iterables,
// ConcurrentModification-free removal through Iterator.remove on a user list, an inner class that
// shadows an outer field, a local class inside a method capturing an effectively final variable,
// and the Iterable.forEach default method with a lambda.
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;

public class InnerClassIterator {
    static class Ring<T> implements Iterable<T> {
        private final Object[] slots;
        private int head;
        private int size;
        private String name = "ring";

        Ring(int capacity) {
            this.slots = new Object[capacity];
        }

        void push(T value) {
            int tail = (head + size) % slots.length;
            if (size == slots.length) {
                slots[head] = value;
                head = (head + 1) % slots.length;
            } else {
                slots[tail] = value;
                size++;
            }
        }

        @SuppressWarnings("unchecked")
        T get(int i) {
            if (i < 0 || i >= size) {
                throw new IndexOutOfBoundsException("index " + i + " for size " + size);
            }
            return (T) slots[(head + i) % slots.length];
        }

        @Override
        public Iterator<T> iterator() {
            return new RingIterator();
        }

        class RingIterator implements Iterator<T> {
            private int next;
            private String name = "iterator";

            @Override
            public boolean hasNext() {
                return next < size;
            }

            @Override
            public T next() {
                if (!hasNext()) {
                    throw new NoSuchElementException("ring " + Ring.this.name + " exhausted");
                }
                return get(next++);
            }

            String describe() {
                return name + " of " + Ring.this.name + " at " + next;
            }
        }
    }

    static class Range implements Iterable<Integer> {
        private final int from;
        private final int to;
        private final int step;

        Range(int from, int to, int step) {
            this.from = from;
            this.to = to;
            this.step = step;
        }

        @Override
        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                private int current = from;

                @Override
                public boolean hasNext() {
                    return step > 0 ? current < to : current > to;
                }

                @Override
                public Integer next() {
                    int value = current;
                    current += step;
                    return value;
                }
            };
        }
    }

    static Iterable<String> words(String sentence) {
        String[] parts = sentence.trim().split(" ");
        return () -> new Iterator<String>() {
            private int i;

            @Override
            public boolean hasNext() {
                return i < parts.length;
            }

            @Override
            public String next() {
                return parts[i++].toUpperCase();
            }
        };
    }

    public static void main(String[] args) {
        Ring<String> ring = new Ring<>(3);
        for (String s : List.of("a", "b", "c", "d", "e")) {
            ring.push(s);
        }
        StringBuilder sb = new StringBuilder();
        for (String s : ring) {
            sb.append(s).append(' ');
        }
        System.out.println("ring contents: " + sb.toString().trim());

        Ring<String>.RingIterator it = ring.new RingIterator();
        System.out.println(it.describe() + ": " + it.next() + it.next());
        System.out.println(it.describe());
        it.next();
        try {
            it.next();
        } catch (NoSuchElementException e) {
            System.out.println("caught: " + e.getMessage());
        }

        int total = 0;
        for (int v : new Range(0, 20, 3)) {
            total += v;
        }
        System.out.println("range sum: " + total);
        List<Integer> down = new ArrayList<>();
        new Range(10, 0, -4).forEach(down::add);
        System.out.println("range down: " + down);

        for (String w : words("the quick  brown fox")) {
            System.out.print("[" + w + "]");
        }
        System.out.println();

        String prefix = "item-";
        class Labeler {
            String label(int n) {
                return prefix + n * n;
            }
        }
        Labeler labeler = new Labeler();
        List<String> labels = new ArrayList<>();
        for (int n = 1; n <= 4; n++) {
            labels.add(labeler.label(n));
        }
        System.out.println(labels);

        List<Integer> numbers = new ArrayList<>(List.of(5, 12, 8, 3, 20, 7));
        Iterator<Integer> iter = numbers.iterator();
        while (iter.hasNext()) {
            if (iter.next() % 2 == 1) {
                iter.remove();
            }
        }
        System.out.println("evens: " + numbers);
        System.out.println("ring get(1): " + ring.get(1));
        try {
            System.out.println("ring get(5): " + ring.get(5));
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of range: " + e.getMessage());
        }
    }
}
