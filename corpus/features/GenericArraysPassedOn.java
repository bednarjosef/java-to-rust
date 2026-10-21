// The array of a generic method's type variable passed on: to a generic class's constructor that
// keeps it (`Arrays.asList` making its list), to another generic method (a sort's `swap`), and a
// bounded `<T extends Comparable<T>> T[] sort(T[])` given an `Integer[]` and returning it.
public class GenericArraysPassedOn {
    static class Fixed<E> {
        final E[] items;

        Fixed(E[] items) {
            this.items = items;
        }

        E get(int i) {
            return items[i];
        }

        int size() {
            return items.length;
        }
    }

    @SafeVarargs
    static <T> Fixed<T> fixed(T... a) {
        return new Fixed<>(a);
    }

    static <T> T either(boolean first, T a, T b) {
        return first ? a : b;
    }

    static <T> void reverse(T[] a, int lo, int hi) {
        hi--;
        while (lo < hi) {
            T t = a[lo];
            a[lo++] = a[hi];
            a[hi--] = t;
        }
    }

    static <T> void flip(T[] a) {
        reverse(a, 0, a.length);
    }

    static <T extends Comparable<T>> T[] sort(T[] array) {
        for (int i = 1; i < array.length; i++) {
            for (int j = i; j > 0 && array[j].compareTo(array[j - 1]) < 0; j--) {
                swap(array, j, j - 1);
            }
        }
        return array;
    }

    static <T> void swap(T[] a, int i, int j) {
        T t = a[i];
        a[i] = a[j];
        a[j] = t;
    }

    public static void main(String[] args) {
        Fixed<Integer> f = fixed(3, 1, 2);
        System.out.println(f.get(0) + " " + f.get(2) + " " + f.size());
        Object[] objects = {"a", 1, 'c'};
        flip(objects);
        System.out.println(objects[0] + " " + objects[1] + " " + objects[2]);
        Integer[] numbers = {5, 3, 9, 1, -4};
        Integer[] sorted = sort(numbers);
        System.out.println(sorted[0] + " " + sorted[4] + " " + (sorted == numbers));
        Double[] halves = sort(new Double[] {2.5, -0.5, 1.0});
        System.out.println(halves[0] + " " + halves[2]);
        System.out.println(either(true, "x", "y") + either(false, 1, 2));
    }
}
