import java.util.Arrays;

// `Arrays.copyOf(a, n, type)` and `copyOfRange(a, from, to, type)` with the array's own class, as
// `ArrayList` and `toArray` call them: the same copies as `copyOf(a, n)` and
// `copyOfRange(a, from, to)`, which take arrays of references too. Into `Object[].class` from an
// array of other references, the copy is an `Object[]` that takes any object.
public class ArraysCopyOfWithClass {
    static class Task {
        final int n;

        Task(int n) {
            this.n = n;
        }

        public String toString() {
            return "T" + n;
        }
    }

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
        Task[] tasks = {new Task(1), new Task(2), new Task(3)};
        Object[] copied = Arrays.copyOf(tasks, 2, Object[].class);
        copied[1] = "other";
        System.out.println(copied.length + " " + copied[0] + " " + copied[1] + " " + tasks[1]);
        System.out.println(copied.getClass() == Object[].class);
    }
}
