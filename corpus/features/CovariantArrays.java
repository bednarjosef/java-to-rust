// Covariant arrays: a `Version[]` passed where an `Object[]` or an erased `T[]` is expected, and
// an `Integer[]` where the `T[]` of `T extends Comparable<T>` is a `Comparable[]`, stays the same
// array, so `toArray(a)` with an array big enough fills and returns `a` itself, and
// `toArray(new Version[0])` returns a new array of versions.
import java.util.Arrays;

public class CovariantArrays {
    static class Version implements Comparable<Version> {
        final int major;
        final int minor;

        Version(int major, int minor) {
            this.major = major;
            this.minor = minor;
        }

        public int compareTo(Version o) {
            return major != o.major ? Integer.compare(major, o.major) : Integer.compare(minor, o.minor);
        }

        public String toString() { return major + "." + minor; }
    }

    static class Bag<E> {
        private Object[] items = new Object[2];
        private int size;

        void add(E e) {
            if (size == items.length) {
                items = Arrays.copyOf(items, size * 2);
            }
            items[size++] = e;
        }

        @SuppressWarnings("unchecked")
        <T> T[] toArray(T[] a) {
            if (a.length < size) {
                return (T[]) Arrays.copyOf(items, size, a.getClass());
            }
            System.arraycopy(items, 0, a, 0, size);
            if (a.length > size) {
                a[size] = null;
            }
            return a;
        }
    }

    static int count(Object[] things) {
        int n = 0;
        for (Object thing : things) {
            if (thing != null) {
                n++;
            }
        }
        return n;
    }

    static void sort(Version[] versions) {
        for (int i = 1; i < versions.length; i++) {
            Version v = versions[i];
            int j = i - 1;
            while (j >= 0 && versions[j].compareTo(v) > 0) {
                versions[j + 1] = versions[j];
                j--;
            }
            versions[j + 1] = v;
        }
    }

    static <T extends Comparable<T>> int countGreater(T[] array, T pivot) {
        int count = 0;
        for (T item : array) {
            if (item.compareTo(pivot) > 0) {
                count++;
            }
        }
        return count;
    }

    static <T> T first(T[] items) {
        return items[0];
    }

    static void fill(Object[] slots, Object value) {
        for (int i = 0; i < slots.length; i++) {
            slots[i] = value;
        }
    }

    public static void main(String[] args) {
        Bag<Version> bag = new Bag<>();
        bag.add(new Version(2, 0));
        bag.add(new Version(1, 4));
        bag.add(new Version(1, 12));

        Version[] fresh = bag.toArray(new Version[0]);
        sort(fresh);
        System.out.println(fresh[0] + " " + fresh[1] + " " + fresh[2] + " " + fresh.length);

        Version[] big = new Version[5];
        big[4] = new Version(9, 9);
        Version[] same = bag.toArray(big);
        System.out.println((same == big) + " " + count(big) + " " + big[3] + " " + big[4]);
        System.out.println(first(big).major + first(fresh).minor);

        Version[] slots = new Version[3];
        fill(slots, new Version(0, 1));
        System.out.println(slots[0] + " " + (slots[0] == slots[2]) + " " + count(slots));
        Object[] view = slots;
        view[1] = new Version(3, 3);
        System.out.println(slots[1].compareTo(slots[0]) + " " + view.length);

        Object[] mixed = {"text", null, slots[1]};
        System.out.println(count(mixed) + " " + mixed[0] + " " + mixed[2]);
        for (Version v : fresh) {
            System.out.print(v.minor + " ");
        }
        System.out.println();

        Integer[] boxed = {5, 1, 9, 3, 7};
        System.out.println(countGreater(boxed, 4) + " " + countGreater(fresh, new Version(1, 5)));
    }
}
