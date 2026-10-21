import java.util.ConcurrentModificationException;
import java.util.Iterator;
import java.util.NoSuchElementException;

public class IteratorRemove {
    // An iterator that can remove the element it returned last, as an interface of the program:
    // Iterator.remove is a default method of the JDK's.
    interface RemovingIterator<E> extends Iterator<E> {
        void remove();
    }

    // A list of ints whose iterator supports `remove` and fails fast on other changes, as the
    // JDK's lists do.
    static class Numbers implements Iterable<Integer> {
        private int[] items = new int[2];
        private int size;
        private int changes;

        void add(int x) {
            if (size == items.length) {
                int[] bigger = new int[size * 2];
                for (int i = 0; i < size; i++) bigger[i] = items[i];
                items = bigger;
            }
            items[size++] = x;
            changes++;
        }

        int size() { return size; }

        void removeLast() {
            size--;
            changes++;
        }

        public RemovingIterator<Integer> iterator() {
            return new RemovingIterator<Integer>() {
                int next;
                int last = -1;
                int expected = changes;

                public boolean hasNext() { return next < size; }

                public Integer next() {
                    if (expected != changes) throw new ConcurrentModificationException();
                    if (next >= size) throw new NoSuchElementException();
                    last = next;
                    return items[next++];
                }

                public void remove() {
                    if (last < 0) throw new IllegalStateException();
                    for (int i = last; i < size - 1; i++) items[i] = items[i + 1];
                    size--;
                    changes++;
                    expected = changes;
                    next = last;
                    last = -1;
                }
            };
        }

        String show() {
            StringBuilder sb = new StringBuilder("[");
            for (int x : this) {
                if (sb.length() > 1) sb.append(", ");
                sb.append(x);
            }
            return sb.append(']').toString();
        }
    }

    public static void main(String[] args) {
        Numbers ns = new Numbers();
        for (int i = 1; i <= 9; i++) ns.add(i * i % 7);
        System.out.println(ns.show());

        // Removing the even numbers through the iterator, from a while loop.
        RemovingIterator<Integer> it = ns.iterator();
        while (it.hasNext()) {
            if (it.next() % 2 == 0) it.remove();
        }
        System.out.println(ns.show() + " size " + ns.size());

        // Changing the list inside a for-each fails on the next element.
        try {
            for (int x : ns) {
                if (x == 1) ns.add(100);
            }
            System.out.println("no failure");
        } catch (ConcurrentModificationException e) {
            System.out.println("concurrent modification, size " + ns.size());
        }

        // Removing through the list at the second-to-last element ends the loop without a
        // failure, as with the JDK's lists: hasNext is false before next can check.
        for (int i = 0; i < 4; i++) ns.add(i);
        int seen = 0;
        for (int x : ns) {
            seen++;
            if (seen == ns.size() - 1) ns.removeLast();
        }
        System.out.println("seen " + seen + " of " + ns.size());

        try {
            RemovingIterator<Integer> fresh = ns.iterator();
            fresh.remove();
        } catch (IllegalStateException e) {
            System.out.println("remove before next");
        }
        Iterator<Integer> done = ns.iterator();
        while (done.hasNext()) done.next();
        try {
            done.next();
        } catch (NoSuchElementException e) {
            System.out.println("no more elements");
        }
    }
}
