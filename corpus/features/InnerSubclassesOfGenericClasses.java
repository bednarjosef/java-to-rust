// Inner classes of a generic class that extend each other, as the JDK's AbstractList.ListItr
// extends Itr: javac names the superclass `Seq<E>.Itr`, an inner class of a parameterized type.
public class InnerSubclassesOfGenericClasses {
    interface Cursor<E> {
        boolean hasNext();
        E next();
    }

    interface TwoWayCursor<E> extends Cursor<E> {
        boolean hasPrevious();
        E previous();
        int nextIndex();
    }

    static class Seq<E> {
        private final Object[] items;
        private int size;
        int changes;

        Seq(int capacity) { items = new Object[capacity]; }

        void add(E e) {
            items[size++] = e;
            changes++;
        }

        @SuppressWarnings("unchecked")
        E get(int i) { return (E) items[i]; }

        int size() { return size; }

        Cursor<E> cursor() { return new Itr(); }

        TwoWayCursor<E> cursor(int index) { return new ListItr(index); }

        Cursor<E> slice(int from, int to) { return new Slice(from, to); }

        // The superclass: it reads the outer object's fields and methods.
        private class Itr implements Cursor<E> {
            int cursor;
            int expected = changes;

            public boolean hasNext() { return cursor != size(); }

            public E next() {
                if (expected != changes) throw new IllegalStateException("changed");
                return get(cursor++);
            }
        }

        // The subclass, with a constructor of its own and no `super(..)`.
        private class ListItr extends Itr implements TwoWayCursor<E> {
            ListItr(int index) { cursor = index; }

            public boolean hasPrevious() { return cursor != 0; }

            public E previous() { return get(--cursor); }

            public int nextIndex() { return cursor; }
        }

        // A superclass whose constructor takes arguments, and a subclass passing them on.
        private class Range extends Itr {
            final int end;

            Range(int from, int end) {
                this.cursor = from;
                this.end = end;
            }

            @Override
            public boolean hasNext() { return cursor < end && super.hasNext(); }
        }

        private class Slice extends Range {
            Slice(int from, int to) { super(from, Math.min(to, size)); }

            @Override
            public E next() { return get(cursor++); }
        }
    }

    static <E> String all(Cursor<E> c) {
        StringBuilder sb = new StringBuilder();
        while (c.hasNext()) sb.append(c.next()).append(' ');
        return sb.toString().trim();
    }

    public static void main(String[] args) {
        Seq<String> words = new Seq<>(8);
        for (String w : "one two three four five".split(" ")) words.add(w);
        System.out.println(all(words.cursor()));
        TwoWayCursor<String> c = words.cursor(2);
        System.out.println(c.nextIndex() + " " + c.next() + " " + c.next());
        while (c.hasPrevious()) System.out.print(c.previous() + " ");
        System.out.println();
        System.out.println(all(words.slice(1, 3)));
        System.out.println(all(words.slice(3, 10)));
        Cursor<String> stale = words.cursor(0);
        words.add("six");
        try {
            stale.next();
        } catch (IllegalStateException e) {
            System.out.println("stale: " + e.getMessage());
        }
        Seq<Integer> numbers = new Seq<>(4);
        numbers.add(7);
        numbers.add(9);
        int sum = 0;
        for (Cursor<Integer> n = numbers.cursor(); n.hasNext(); ) sum += n.next();
        System.out.println(sum);
    }
}
