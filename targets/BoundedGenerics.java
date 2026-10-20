// Features: bounded type parameters <T extends Comparable<T>>, generic static methods, wildcards
// (List<? extends Number>, Comparable<? super T>), a user class implementing Comparable, generic
// max/min/clamp/binarySearch helpers, varargs generics, Number.doubleValue, Integer.parseInt.
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class BoundedGenerics {
    static final class Version implements Comparable<Version> {
        final int major;
        final int minor;

        Version(int major, int minor) {
            this.major = major;
            this.minor = minor;
        }

        static Version parse(String text) {
            String[] parts = text.split("\\.");
            return new Version(Integer.parseInt(parts[0]), Integer.parseInt(parts[1]));
        }

        @Override
        public int compareTo(Version other) {
            if (major != other.major) {
                return major - other.major;
            }
            return minor - other.minor;
        }

        @Override
        public String toString() {
            return "v" + major + "." + minor;
        }
    }

    static <T extends Comparable<T>> T max(List<T> items) {
        if (items.isEmpty()) {
            throw new IllegalArgumentException("empty list");
        }
        T best = items.get(0);
        for (T item : items) {
            if (item.compareTo(best) > 0) {
                best = item;
            }
        }
        return best;
    }

    @SafeVarargs
    static <T extends Comparable<? super T>> T min(T first, T... rest) {
        T best = first;
        for (T item : rest) {
            if (item.compareTo(best) < 0) {
                best = item;
            }
        }
        return best;
    }

    static <T extends Comparable<T>> T clamp(T value, T low, T high) {
        if (value.compareTo(low) < 0) {
            return low;
        }
        if (value.compareTo(high) > 0) {
            return high;
        }
        return value;
    }

    static double sum(List<? extends Number> numbers) {
        double total = 0;
        for (Number n : numbers) {
            total += n.doubleValue();
        }
        return total;
    }

    static void fillIntegers(List<? super Integer> target, int count) {
        for (int i = 1; i <= count; i++) {
            target.add(i * 3);
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

    static <T extends Comparable<T>> boolean isSorted(List<T> items) {
        for (int i = 1; i < items.size(); i++) {
            if (items.get(i - 1).compareTo(items.get(i)) > 0) {
                return false;
            }
        }
        return true;
    }

    public static void main(String[] args) {
        List<Integer> ints = new ArrayList<>(List.of(4, 19, -2, 11));
        System.out.println("max int: " + max(ints));
        List<String> words = List.of("pear", "apple", "zucchini", "fig");
        System.out.println("max word: " + max(words));

        List<Version> releases = new ArrayList<>();
        for (String text : new String[] {"1.4", "2.0", "1.12", "2.10", "0.9"}) {
            releases.add(Version.parse(text));
        }
        System.out.println("latest: " + max(releases));
        System.out.println("sorted? " + isSorted(releases));
        Arrays.sort(releases.toArray(new Version[0]));
        List<Version> ordered = new ArrayList<>(releases);
        ordered.sort(null);
        System.out.println("ordered: " + ordered + " sorted? " + isSorted(ordered));

        System.out.println("min of strings: " + min("kiwi", "banana", "cherry"));
        System.out.println("min of one: " + min(7));
        System.out.println("clamp 15 into [0,10]: " + clamp(15, 0, 10));
        System.out.println("clamp 'm' into [a,f]: " + clamp("m", "a", "f"));
        System.out.println("clamp version: " + clamp(Version.parse("1.5"), Version.parse("1.0"), Version.parse("1.9")));

        List<Integer> small = new ArrayList<>();
        fillIntegers(small, 4);
        List<Object> objects = new ArrayList<>();
        fillIntegers(objects, 2);
        System.out.println("filled: " + small + " and " + objects);
        System.out.println("sum of ints: " + sum(small) + ", of doubles: " + sum(List.of(1.5, 2.25)));

        Integer[] boxed = {5, 1, 9, 3, 7};
        System.out.println("greater than 4: " + countGreater(boxed, 4));
        System.out.println("max empty: ");
        try {
            max(new ArrayList<Integer>());
        } catch (IllegalArgumentException e) {
            System.out.println("  " + e.getMessage());
        }
    }
}
