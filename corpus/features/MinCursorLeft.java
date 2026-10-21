public class MinCursorLeft {
    static class TreeNode {
        int key;
        TreeNode left;
        TreeNode(int key) { this.key = key; }
    }

    static TreeNode min(TreeNode t) {
        while (t.left != null) t = t.left;
        return t;
    }

    public static void main(String[] args) {
        TreeNode root = new TreeNode(5);
        root.left = new TreeNode(3);
        root.left.left = new TreeNode(1);
        System.out.println(min(root).key);
    }
}
