class Tree {
    int key;
    int count;
    Tree left, right;
    Tree(int key) { this.key = key; count = 1; }
    void insert(int k) {
        if (k == key) { count++; return; }
        if (k < key) {
            if (left == null) left = new Tree(k); else left.insert(k);
        } else {
            if (right == null) right = new Tree(k); else right.insert(k);
        }
    }
    int size() {
        int s = count;
        if (left != null) s += left.size();
        if (right != null) s += right.size();
        return s;
    }
    int height() {
        int l = left == null ? 0 : left.height();
        int r = right == null ? 0 : right.height();
        return 1 + Math.max(l, r);
    }
    Tree find(int k) {
        Tree t = this;
        while (t != null && t.key != k) t = k < t.key ? t.left : t.right;
        return t;
    }
    public String toString() { return "(" + left + " " + key + "x" + count + " " + right + ")"; }
}
class Pair {
    final Tree a;
    Tree b;
    Pair(Tree a, Tree b) { this.a = a; this.b = b; }
    boolean same() { return a == b; }
}
public class SearchTree {
    static Tree build(int[] keys) {
        Tree root = null;
        for (int k : keys) {
            if (root == null) root = new Tree(k); else root.insert(k);
        }
        return root;
    }
    public static void main(String[] args) {
        Tree t = build(new int[] {5, 3, 8, 3, 1, 9, 7, 5, 5});
        System.out.println(t);
        System.out.println(t.size() + " " + t.height());
        Tree f = t.find(3);
        if (f != null) System.out.println("found " + f.key + " " + f.count);
        Tree m = t.find(4);
        System.out.println(m == null ? "missing" : "found");
        Pair p = new Pair(t, f);
        System.out.println(p.same());
        p.b = t;
        System.out.println(p.same() + " " + (p.a == p.b));
        p.b = null;
        System.out.println(p.b);
        f.count += 10;
        System.out.println(t.find(3).count);
        Tree empty = build(new int[0]);
        System.out.println(empty);
        System.out.println(empty.size());
    }
}
