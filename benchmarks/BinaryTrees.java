// Builds complete binary trees of many depths, walks each one and drops it: object allocation
// and recursion, with a long-lived tree kept alive throughout.
public class BinaryTrees {
    static class Node {
        Node left;
        Node right;

        Node(Node left, Node right) {
            this.left = left;
            this.right = right;
        }
    }

    static Node build(int depth) {
        if (depth == 0) {
            return new Node(null, null);
        }
        return new Node(build(depth - 1), build(depth - 1));
    }

    static int count(Node node) {
        if (node.left == null) {
            return 1;
        }
        return 1 + count(node.left) + count(node.right);
    }

    public static void main(String[] args) {
        int maxDepth = 20;
        Node longLived = build(maxDepth);
        for (int depth = 4; depth <= maxDepth; depth += 2) {
            int iterations = 1 << (maxDepth - depth + 4);
            int sum = 0;
            for (int i = 0; i < iterations; i++) {
                sum += count(build(depth));
            }
            System.out.println(iterations + " trees of depth " + depth + " nodes " + sum);
        }
        System.out.println("long lived tree of depth " + maxDepth + " nodes " + count(longLived));
    }
}
