// A variable assigned inside a null test whose operand is never null, as in the JDK's
// `Spliterators.IteratorSpliterator.forEachRemaining` (`if ((i = it) == null) { .. }`): the test
// is false, but the assignment still happens, and the variable is used after it.
public class AssignedInNullTest {
    static final class Node {
        final String name;

        Node(String name) {
            this.name = name;
        }
    }

    static final class Holder {
        final Node node;
        int fallbacks;

        Holder(String name) {
            node = new Node(name);
        }

        String describe() {
            Node n;
            if ((n = node) == null) {
                n = new Node("fallback");
                fallbacks++;
            }
            return n.name + " " + fallbacks;
        }
    }

    public static void main(String[] args) {
        Holder h = new Holder("first");
        System.out.println(h.describe());
        System.out.println(new Holder("second").describe());
    }
}
