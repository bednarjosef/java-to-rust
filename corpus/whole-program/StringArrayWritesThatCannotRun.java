import java.util.Arrays;

// An `Arrays.sort` of a `String[]` that is also an `Object[]` (passed for the array of a type
// variable beside an `Integer[]`), in a method that reachability keeps but that cannot run: its
// class has objects, but none is ever the receiver of the call. The same calls on an `Integer[]`
// reach the JDK's methods for an `Object[]` anyway, so the translation is lowered with the writes
// first, finds that none can run, and is lowered again without them (`Reach::string_array_writes`).
public class StringArrayWritesThatCannotRun {
    interface Arranger {
        String arrange(String[] words);
    }

    static class Sorting implements Arranger {
        public String arrange(String[] words) {
            Arrays.sort(words);
            return String.join(",", words);
        }

        public String toString() {
            return "sorting";
        }
    }

    static class Reversing implements Arranger {
        public String arrange(String[] words) {
            StringBuilder out = new StringBuilder();
            for (int i = words.length - 1; i >= 0; i--) {
                out.append(words[i]);
                if (i > 0) {
                    out.append(',');
                }
            }
            return out.toString();
        }
    }

    static <T> T last(T[] a) {
        return a[a.length - 1];
    }

    public static void main(String[] args) {
        String[] words = {"pear", "apple", "fig"};
        Integer[] numbers = {3, 1, 2};
        System.out.println(last(words) + " " + last(numbers));
        Arrays.sort(numbers);
        Arrays.fill(numbers, 0, 1, 7);
        System.out.println(Arrays.toString(numbers));
        System.out.println(new Sorting());
        Arranger arranger = new Reversing();
        System.out.println(arranger.arrange(words) + " " + last(words));
    }
}
