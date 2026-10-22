import java.util.Arrays;
import java.util.StringJoiner;

// `StringJoiner` keeps its elements in a `String[]`, which the program's use of a `String[]` as
// an `Object[]` makes an `Object[]` everywhere: joining reads its elements as strings again.
public class CovariantStringJoiner {
    static int count(Object[] values) {
        return values.length;
    }

    public static void main(String[] args) {
        String[] words = {"alpha", "beta", "gamma"};
        Object[] objects = words;
        System.out.println(count(objects) + " " + Arrays.toString(objects));
        StringJoiner joiner = new StringJoiner(", ", "[", "]");
        joiner.setEmptyValue("nothing");
        System.out.println(joiner);
        for (String word : words) {
            joiner.add(word);
        }
        joiner.add(null);
        System.out.println(joiner + " " + joiner.length());
        StringJoiner other = new StringJoiner("-");
        other.add("x").add("y");
        System.out.println(joiner.merge(other));
    }
}
