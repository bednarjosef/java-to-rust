// Objects passed to code that calls a method on them: whether one stays on the stack depends on
// what its own class's method does with `this`, not on what other classes' methods do.
public class StackReceivers {
    interface Fn {
        int apply(int x);
    }

    // Lets its `this` escape: its objects stay in the heap.
    static class Keeper implements Fn {
        static Fn kept;
        int calls;
        public int apply(int x) {
            kept = this;
            calls++;
            return x;
        }
    }

    static class Twice implements Fn {
        public int apply(int x) { return 2 * x; }
    }

    static int sum(int[] xs, Fn f) {
        int total = 0;
        for (int x : xs) total += f.apply(x);
        return total;
    }

    // A visitor whose objects travel through the nodes' `accept`.
    abstract static class Node {
        abstract int accept(Visitor v);
    }

    static class Leaf extends Node {
        final int value;
        Leaf(int value) { this.value = value; }
        int accept(Visitor v) { return v.visitLeaf(this); }
    }

    static class Pair extends Node {
        final Node left, right;
        Pair(Node left, Node right) { this.left = left; this.right = right; }
        int accept(Visitor v) { return v.visitPair(this); }
    }

    abstract static class Visitor {
        abstract int visitLeaf(Leaf leaf);
        int visitPair(Pair pair) { return pair.left.accept(this) + pair.right.accept(this); }
    }

    static class Summer extends Visitor {
        int visitLeaf(Leaf leaf) { return leaf.value; }
    }

    // Records itself: its objects escape.
    static class Recorder extends Visitor {
        static Visitor last;
        static int seen;
        int visitLeaf(Leaf leaf) {
            last = this;
            seen++;
            return 1;
        }
    }

    interface Mapper<A, B> {
        B apply(A a);
    }

    static <A, B> Object[] map(A[] xs, Mapper<A, B> f) {
        Object[] out = new Object[xs.length];
        for (int i = 0; i < xs.length; i++) out[i] = f.apply(xs[i]);
        return out;
    }

    public static void main(String[] args) {
        int[] xs = { 1, 2, 3, 4 };
        long total = 0;
        for (int round = 0; round < 1000; round++) {
            final int k = round % 7;
            total += sum(xs, new Twice());
            total += sum(xs, x -> x + k);
            total += sum(xs, new Keeper());
        }
        System.out.println(total + " " + ((Keeper) Keeper.kept).calls);

        Node tree = new Pair(new Pair(new Leaf(1), new Leaf(2)), new Leaf(3));
        long visited = 0;
        for (int round = 0; round < 1000; round++) {
            visited += tree.accept(new Summer());
            visited += tree.accept(new Recorder());
        }
        System.out.println(visited + " " + Recorder.seen);

        String[] words = { "a", "bb", "ccc" };
        int mapped = 0;
        for (int round = 0; round < 100; round++) {
            final int r = round;
            for (Object y : map(words, w -> w.length() * r)) mapped += (Integer) y;
        }
        System.out.println(mapped);
    }
}
