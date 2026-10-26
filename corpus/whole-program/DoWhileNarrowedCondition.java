// A do-while whose body returns when a variable is null and whose condition reads it, as
// netty's ConcurrentSkipListIntObjMultimap.pollCeilingEntry does.
public class DoWhileNarrowedCondition {
    static final class Node { final int key; String val; Node(int key, String val) { this.key = key; this.val = val; } }
    Node[] nodes = { new Node(1, null), new Node(2, "b"), new Node(3, "c") };
    int at;
    Node findNear() { return at < nodes.length ? nodes[at++] : null; }
    boolean remove(int key, String val) { return key != 2; }
    String pollCeiling() {
        Node node;
        String val;
        do {
            node = findNear();
            if (node == null) {
                return null;
            }
            val = node.val;
        } while (val == null || !remove(node.key, val));
        return node.key + "=" + val;
    }
    public static void main(String[] args) {
        DoWhileNarrowedCondition m = new DoWhileNarrowedCondition();
        System.out.println(m.pollCeiling());
        System.out.println(m.pollCeiling());
    }
}
