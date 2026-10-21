import java.util.Arrays;

// `Arrays.sort` and `Arrays.fill` of a `String[]` that is also an `Object[]`, because it is passed
// for the array of a type variable beside an `Integer[]`: they run the JDK's methods for an
// `Object[]` (`ComparableTimSort`, the fill loop), which change the array itself.
public class StringArraySortsAsObjectArrays {
    static <T> T last(T[] a) {
        return a[a.length - 1];
    }

    public static void main(String[] args) {
        String[] words = {"pear", "apple", "fig", "banana", "cherry"};
        Integer[] numbers = {3, 1, 2};
        System.out.println(last(words) + " " + last(numbers));
        Arrays.sort(words);
        System.out.println(String.join(",", words) + " " + last(words));
        Arrays.sort(words, 1, 4);
        Arrays.fill(words, 0, 2, "kiwi");
        String[] blanks = new String[2];
        Arrays.fill(blanks, "-");
        System.out.println(Arrays.toString(words) + " " + Arrays.toString(blanks) + " " + last(blanks));
    }
}
