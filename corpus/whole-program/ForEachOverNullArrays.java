import java.util.List;

// A for-each loop reads the length of its array as it starts, which throws a catchable
// `NullPointerException` for a null array, with the JVM's message: javac copies the array into a
// local of its own, which the message names by its slot.
public class ForEachOverNullArrays {
    static int[] none;
    String[] names;

    static int sum(int[] values) {
        int total = 0;
        for (int v : values) {
            total += v;
        }
        return total;
    }

    static String[] maybe(boolean given) {
        return given ? new String[] {"a", "b"} : null;
    }

    public static void main(String[] args) {
        int[] local = args.length > 5 ? new int[] {1} : null;
        try {
            for (int x : local) {
                System.out.println(x);
            }
        } catch (NullPointerException e) {
            System.out.println("local: " + e.getMessage());
        }
        try {
            for (int x : none) {
                System.out.println(x);
            }
        } catch (Exception e) {
            System.out.println("static: " + e.getMessage());
        }
        try {
            for (String s : new ForEachOverNullArrays().names) {
                System.out.println(s);
            }
        } catch (Throwable e) {
            System.out.println("field: " + e.getMessage());
        }
        try {
            for (String s : maybe(false)) {
                System.out.println(s);
            }
        } catch (NullPointerException e) {
            System.out.println("call: " + e.getMessage());
        }
        for (String s : maybe(true)) {
            System.out.println("given " + s);
        }
        try {
            System.out.println(sum(new int[] {1, 2, 3}) + " " + sum(null));
        } catch (NullPointerException e) {
            System.out.println("parameter: " + e.getMessage());
        }
        List<int[]> rows = java.util.Arrays.asList(new int[] {4}, null);
        int seen = 0;
        try {
            for (int[] row : rows) {
                for (int v : row) {
                    seen += v;
                }
            }
        } catch (NullPointerException e) {
            System.out.println("nested after " + seen + ": " + e.getMessage());
        }
    }
}
