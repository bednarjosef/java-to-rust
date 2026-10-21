// A growable list of a type variable, erased: its elements are `Object`s, read back through
// the checked casts that erasure puts at each use.
public class ArrayListLike {
    static class MyList<E> {
        private Object[] items = new Object[2];
        private int size;

        void add(E e) {
            if (size == items.length) {
                Object[] bigger = new Object[items.length * 2];
                System.arraycopy(items, 0, bigger, 0, size);
                items = bigger;
            }
            items[size++] = e;
        }

        @SuppressWarnings("unchecked")
        E get(int i) {
            return (E) items[i];
        }

        E set(int i, E e) {
            E old = get(i);
            items[i] = e;
            return old;
        }

        int indexOf(Object o) {
            for (int i = 0; i < size; i++) {
                if (o == null ? items[i] == null : o.equals(items[i])) {
                    return i;
                }
            }
            return -1;
        }

        E remove(int i) {
            E old = get(i);
            System.arraycopy(items, i + 1, items, i, size - i - 1);
            items[--size] = null;
            return old;
        }

        int size() { return size; }

        @Override
        public String toString() {
            StringBuilder sb = new StringBuilder("[");
            for (int i = 0; i < size; i++) {
                if (i > 0) sb.append(", ");
                sb.append(items[i]);
            }
            return sb.append("]").toString();
        }
    }

    static class Point {
        final int x, y;
        Point(int x, int y) { this.x = x; this.y = y; }
        @Override public boolean equals(Object o) { return o instanceof Point p && p.x == x && p.y == y; }
        @Override public int hashCode() { return 31 * x + y; }
        @Override public String toString() { return "(" + x + "," + y + ")"; }
    }

    static <T> int count(MyList<T> list, T x) {
        int n = 0;
        for (int i = 0; i < list.size(); i++) {
            if (list.get(i).equals(x)) n++;
        }
        return n;
    }

    public static void main(String[] args) {
        MyList<String> words = new MyList<>();
        words.add("one");
        words.add("two");
        words.add("three");
        words.add("two");
        System.out.println(words + " " + words.size() + " " + words.indexOf("two") + " " + count(words, "two"));
        String removed = words.remove(1);
        System.out.println(removed + " " + words + " " + words.get(0).length());
        words.set(0, "uno");
        System.out.println(words);

        MyList<Integer> numbers = new MyList<>();
        for (int i = 0; i < 6; i++) numbers.add(i * i);
        int total = 0;
        for (int i = 0; i < numbers.size(); i++) total += numbers.get(i);
        System.out.println(numbers + " total " + total + " " + numbers.indexOf(16));

        MyList<Point> points = new MyList<>();
        points.add(new Point(1, 2));
        points.add(new Point(3, 4));
        System.out.println(points + " " + points.indexOf(new Point(3, 4)) + " " + points.get(1).x);
    }
}
