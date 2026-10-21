import java.util.StringJoiner;

// The JDK's own `toString`s called explicitly inside a `try` that catches runtime exceptions:
// they throw only for bugs, as when the JDK shows an object's text, so the `catch` is for the
// other code in the block.
public class JdkToStrings {
    public static void main(String[] args) {
        StringJoiner joiner = new StringJoiner(", ", "[", "]");
        joiner.add("a").add("b");
        StringJoiner empty = new StringJoiner("-");
        empty.setEmptyValue("nothing");
        for (String text : new String[] {"7", "nope"}) {
            try {
                String shown = joiner.toString() + " " + empty.toString();
                System.out.println(shown + " " + Integer.parseInt(text));
            } catch (RuntimeException e) {
                System.out.println("caught " + e.getMessage());
            }
        }
    }
}
