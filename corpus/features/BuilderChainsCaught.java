// A builder's `insert` and `deleteCharAt` called on a new builder, with an index that may be out
// of range, inside a `try` that catches it: the call runs where the `catch` sees its exception,
// also as the receiver of a further call and as printed text.
public class BuilderChainsCaught {
    static String insert(String s, int at) {
        try {
            return new StringBuilder(s).insert(at, "x").append('!').toString();
        } catch (StringIndexOutOfBoundsException e) {
            return "insert out of range";
        }
    }

    static void delete(String s, int at) {
        try {
            System.out.println(new StringBuilder(s).deleteCharAt(at));
        } catch (RuntimeException e) {
            System.out.println("delete failed: " + e.getClass().getSimpleName());
        }
    }

    public static void main(String[] args) {
        System.out.println(insert("abc", 1));
        System.out.println(insert("abc", 7));
        delete("abc", 2);
        delete("abc", 3);
    }
}
