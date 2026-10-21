// A varargs method that passes its Object... on to String.format and System.out.format, as the
// JDK's LocaleResources.trace does: the array is the arguments themselves.
public class FormatObjectArray {
    static String describe(String format, Object... params) {
        return String.format(format, params);
    }

    static void trace(String format, Object... params) {
        System.out.format(format, params);
    }

    public static void main(String[] args) {
        System.out.println(describe("%s has %s items%n", "cart", 3));
        System.out.println(describe("[%S] [%s]", "loud", null));
        trace("%s and %s%n", "first", 2.5);
        Object[] values = {"x", 'y', true};
        System.out.println(String.format("%s-%s-%s", values));
        String[] names = {"ann", "bob"};
        System.out.println(String.format("%s & %s", (Object[]) names));
        System.out.println(describe("100%% %s", "done"));
    }
}
