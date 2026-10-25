import java.util.Arrays;

// `Arrays.sort` and `Arrays.fill` of a `String[]` that is also an `Object[]`, beside the same calls
// on an `Integer[]`, which run the JDK's methods for an `Object[]` anyway: reaching the JDK's
// methods for the `String[]` too reaches nothing new, so the program is lowered with them over the
// units and reach of its one load, and as they can run, that is the translation
// (`Reach::string_array_writes`).
public class StringArraySortsBesideObjectArraySorts {
    static <T> T last(T[] a) {
        return a[a.length - 1];
    }

    public static void main(String[] args) {
        String[] words = {"pear", "apple", "fig", "banana", "cherry"};
        Integer[] numbers = {3, 1, 5, 2, 4};
        System.out.println(last(words) + " " + last(numbers));
        Arrays.sort(numbers);
        Arrays.sort(words);
        System.out.println(String.join(",", words) + " " + Arrays.toString(numbers));
        Integer[] reversed = {5, 4, 3, 2, 1};
        Arrays.sort(reversed, 1, 4);
        Arrays.sort(words, 1, 4);
        Arrays.fill(numbers, 0, 2, 9);
        Arrays.fill(words, 0, 2, "kiwi");
        String[] blanks = new String[2];
        Integer[] zeros = new Integer[2];
        Arrays.fill(blanks, "-");
        Arrays.fill(zeros, 0);
        System.out.println(Arrays.toString(words) + " " + Arrays.toString(blanks) + " " + last(blanks));
        System.out.println(Arrays.toString(numbers) + " " + Arrays.toString(reversed) + " " + last(zeros));
    }
}
