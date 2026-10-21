import java.util.Arrays;

// `Arrays.copyOf(a, n, type)` and `copyOfRange(a, from, to, type)` with the array's own class, as
// `ArrayList` and `toArray` call them: the same copies as `copyOf(a, n)` and
// `copyOfRange(a, from, to)`, which take arrays of references too.
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
        String[] middle = Arrays.copyOfRange(more, 1, 4);
        System.out.println(middle.length + " " + middle[0] + " " + middle[2]);
        Object[] tail = Arrays.copyOfRange(items, 1, 3, (Class<? extends Object[]>) items.getClass());
        System.out.println(tail[0] + " " + tail[1]);
    }
}
