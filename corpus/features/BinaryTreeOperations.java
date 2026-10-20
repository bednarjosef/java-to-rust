public class BinaryTree {
    static class TreeNode {
        int key;
        TreeNode left;
        TreeNode right;
        TreeNode(int key) { this.key = key; }
    }

    static String order = "";

    static TreeNode insert(TreeNode root, int key) {
        if (root == null) return new TreeNode(key);
        if (key < root.key) root.left = insert(root.left, key);
        else if (key > root.key) root.right = insert(root.right, key);
        return root;
    }

    static boolean contains(TreeNode t, int key) {
        if (t == null) return false;
        if (key == t.key) return true;
        return contains(key < t.key ? t.left : t.right, key);
    }

    static int height(TreeNode t) {
        if (t == null) return 0;
        return 1 + Math.max(height(t.left), height(t.right));
    }

    static void inorder(TreeNode t) {
        if (t == null) return;
        inorder(t.left);
        order = order + t.key + " ";
        inorder(t.right);
    }

    static TreeNode min(TreeNode t) {
        return t.left == null ? t : min(t.left);
    }

    static TreeNode remove(TreeNode t, int key) {
        if (t == null) return null;
        if (key < t.key) {
            t.left = remove(t.left, key);
        } else if (key > t.key) {
            t.right = remove(t.right, key);
        } else {
            if (t.left == null) return t.right;
            if (t.right == null) return t.left;
            TreeNode m = min(t.right);
            t.key = m.key;
            t.right = remove(t.right, m.key);
        }
        return t;
    }

    static TreeNode mirror(TreeNode t) {
        if (t == null) return null;
        TreeNode l = mirror(t.right);
        t.right = mirror(t.left);
        t.left = l;
        return t;
    }

    static String sorted(TreeNode root) {
        order = "";
        inorder(root);
        return order;
    }

    public static void main(String[] args) {
        TreeNode root = null;
        int[] keys = { 50, 30, 70, 20, 40, 60, 80, 30, 65 };
        for (int k : keys) root = insert(root, k);
        System.out.println(sorted(root));
        System.out.println("height " + height(root));
        System.out.println(contains(root, 65) + " " + contains(root, 66));
        root = remove(root, 50);
        root = remove(root, 20);
        root = remove(root, 99);
        System.out.println(sorted(root) + " root=" + root.key);
        System.out.println("min " + min(root).key);
        root = mirror(root);
        System.out.println(sorted(root));
        TreeNode single = new TreeNode(1);
        TreeNode none = single.left;
        System.out.println(height(none) + " " + height(single));
        TreeNode alias = root;
        root = null;
        System.out.println(alias.key + " " + (root == null));
    }
}
