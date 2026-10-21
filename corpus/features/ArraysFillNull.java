import java.util.Arrays;

// `Arrays.fill` with null on an array of strings, as `StringJoiner.compactElts` clears the
// elements it has joined: the filled elements are null afterwards.
public class ArraysFillNull {
    public static void main(String[] args) {
        String[] parts = {"a", "b", "c", "d"};
        Arrays.fill(parts, 1, 3, null);
        System.out.println(Arrays.toString(parts));
        String[] all = {"x", "y"};
        Arrays.fill(all, null);
        System.out.println(all[0] == null && all[1] == null);
        Arrays.fill(all, "z");
        System.out.println(String.join("-", all));
    }
}
