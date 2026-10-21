import java.util.Arrays;

// `Arrays.copyOf(a, n, type)` with the array's own class, as `ArrayList` and `toArray` call it:
// the same copy as `copyOf(a, n)`.
public class ArraysCopyOfWithClass {
    public static void main(String[] args) {
        Object[] items = {"a", 1, "c"};
        Object[] longer = Arrays.copyOf(items, 5, Object[].class);
        Object[] same = Arrays.copyOf(items, 2, items.getClass());
        System.out.println(longer.length + " " + same.length);
        for (Object o : longer) {
            System.out.println(o);
        }
        String[] words = {"x", "y"};
        String[] more = Arrays.copyOf(words, 3, String[].class);
        System.out.println(more[0] + more[1] + more[2]);
    }
}
