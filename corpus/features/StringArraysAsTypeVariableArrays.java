import java.util.Arrays;

// A `String[]` passed for the array of a type variable: a bounded `<T extends Comparable<T>> T[]
// sort(T[])` sorting it in place and returning it, a generic `reverse`, and the runtime's string
// builtins on the same arrays (`split`, `String.join`, `Arrays.toString`, `equals`, `hashCode`,
// `copyOf`) and `main`'s `args`, with a null element.
public class StringArraysAsTypeVariableArrays {
    static <T extends Comparable<T>> T[] sort(T[] array) {
        for (int i = 1; i < array.length; i++) {
            for (int j = i; j > 0 && array[j].compareTo(array[j - 1]) < 0; j--) {
                T t = array[j];
                array[j] = array[j - 1];
                array[j - 1] = t;
            }
        }
        return array;
    }

    static <T> void reverse(T[] a) {
        for (int lo = 0, hi = a.length - 1; lo < hi; lo++, hi--) {
            T t = a[lo];
            a[lo] = a[hi];
            a[hi] = t;
        }
    }

    static int count(Object[] values) {
        int n = 0;
        for (Object v : values) {
            if (v != null) {
                n++;
            }
        }
        return n;
    }

    public static void main(String[] args) {
        String[] words = {"pear", "apple", "fig", "banana"};
        String[] sorted = sort(words);
        System.out.println(String.join(",", words) + " " + (sorted == words) + " " + words[0].length());
        reverse(words);
        System.out.println(Arrays.toString(words));
        String[] parts = "c b a".split(" ");
        System.out.println(String.join("", sort(parts)) + " " + parts.length);
        String[] copy = Arrays.copyOf(parts, 4);
        System.out.println(Arrays.toString(copy) + " " + count(copy) + " " + Arrays.equals(parts, copy));
        System.out.println(Arrays.hashCode(parts) == Arrays.hashCode(new String[] {"a", "b", "c"}));
        for (String w : sorted) {
            System.out.print(w.charAt(0));
        }
        System.out.println(" " + args.length + " " + count(args));
        Integer[] numbers = sort(new Integer[] {3, -1, 2});
        System.out.println(numbers[0] + " " + numbers[2]);
    }
}
