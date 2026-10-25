import java.util.Arrays;
import java.util.function.ToIntFunction;

// Unboxing a null that the code does not write as a cast throws a `NullPointerException` with
// Java's message wherever the box is converted: a returned value, a local's initial value, an
// argument, a lambda's result that the JDK's stream asks for, an update of a boxed local.
public class UnboxingNullMessagesWithJdk {
    static int returned(Integer x) {
        return x;
    }

    static long widened(Integer x) {
        return x;
    }

    static void takesInt(int i) {
        System.out.println(i);
    }

    public static void main(String[] args) {
        Integer missing = args.length > 5 ? 3 : null;
        try {
            returned(null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(widened(missing));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            int z = missing;
            System.out.println(z);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            takesInt(missing);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        ToIntFunction<Integer> same = x -> x;
        try {
            System.out.println(same.applyAsInt(missing));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(Arrays.stream(new Integer[] {1, missing}).mapToInt(x -> x).sum());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            Integer w = missing;
            w++;
            System.out.println(w);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
