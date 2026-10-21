// Long chains of objects that go out of scope: a singly linked list, an immutable list, a doubly linked list, a degenerate tree, a chain through two classes, a chain
// through arrays, a chain through an interface with two classes, a chain whose link is a field
// of a superclass, a generic list, and an array of long lists.
// Freeing each chain must not need a stack frame per object. The chains are a few hundred
// thousand objects long: long enough that a recursive drop overflows the 8 MB main stack,
// short enough to keep the memory of a test run small.
public class LongChainsDrop {
    static class Node {
        int value;
        Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    static class Frozen {
        final int value;
        final Frozen rest;

        Frozen(int value, Frozen rest) {
            this.value = value;
            this.rest = rest;
        }
    }

    static class Twin {
        int value;
        Twin prev;
        Twin next;

        Twin(int value) {
            this.value = value;
        }
    }

    static class Tree {
        int key;
        Tree left;
        Tree right;

        Tree(int key) {
            this.key = key;
        }
    }

    static class Outer {
        int value;
        Inner inner;

        Outer(int value) {
            this.value = value;
        }
    }

    static class Inner {
        int value;
        Outer outer;

        Inner(int value) {
            this.value = value;
        }
    }

    static class Branch {
        int value;
        Branch[] children = new Branch[2];

        Branch(int value) {
            this.value = value;
        }
    }

    interface Cell {
        int value();

        Cell rest();
    }

    static class Even implements Cell {
        int value;
        Cell rest;

        Even(int value, Cell rest) {
            this.value = value;
            this.rest = rest;
        }

        public int value() {
            return value;
        }

        public Cell rest() {
            return rest;
        }
    }

    static class Odd implements Cell {
        int value;
        Cell rest;

        Odd(int value, Cell rest) {
            this.value = value;
            this.rest = rest;
        }

        public int value() {
            return value * 3;
        }

        public Cell rest() {
            return rest;
        }
    }

    abstract static class Step {
        int value;
        Step next;

        abstract int weight();
    }

    static class Light extends Step {
        int weight() {
            return value;
        }
    }

    static class Heavy extends Step {
        int extra;

        int weight() {
            return value * 10 + extra;
        }
    }

    static class Generic<T> {
        T item;
        Generic<T> next;

        Generic(T item, Generic<T> next) {
            this.item = item;
            this.next = next;
        }
    }

    static long singlyLinked(int n) {
        Node head = null;
        for (int i = 0; i < n; i++) {
            head = new Node(i % 1000, head);
        }
        long sum = 0;
        for (Node p = head; p != null; p = p.next) {
            sum += p.value;
        }
        return sum;
    }

    static long frozen(int n) {
        Frozen list = null;
        for (int i = 0; i < n; i++) {
            list = new Frozen(i % 7, list);
        }
        long sum = 0;
        for (Frozen p = list; p != null; p = p.rest) {
            sum = sum * 3 + p.value;
            sum %= 1000000007L;
        }
        return sum;
    }

    static long doublyLinked(int n) {
        Twin first = new Twin(0);
        Twin last = first;
        for (int i = 1; i < n; i++) {
            Twin t = new Twin(i % 100);
            t.prev = last;
            last.next = t;
            last = t;
        }
        long sum = 0;
        for (Twin p = last; p != null; p = p.prev) {
            sum += p.value;
        }
        // Clearing the back links leaves a plain chain, which is freed when the method returns.
        for (Twin p = first; p != null; p = p.next) {
            p.prev = null;
        }
        return sum;
    }

    static long degenerateTree(int n) {
        Tree root = new Tree(0);
        Tree t = root;
        for (int i = 1; i < n; i++) {
            Tree child = new Tree(i);
            if (i % 2 == 0) {
                t.left = child;
                t.right = new Tree(-i);
            } else {
                t.right = child;
            }
            t = child;
        }
        long sum = 0;
        Tree p = root;
        while (p != null) {
            sum += p.key;
            p = p.left != null ? p.left : p.right;
        }
        return sum;
    }

    static long twoClasses(int n) {
        Outer head = new Outer(0);
        Outer o = head;
        for (int i = 1; i < n; i++) {
            Inner in = new Inner(i);
            o.inner = in;
            o = new Outer(i * 2);
            in.outer = o;
        }
        long sum = 0;
        for (Outer p = head; p != null; p = p.inner == null ? null : p.inner.outer) {
            sum += p.value;
            if (p.inner != null) {
                sum += p.inner.value;
            }
        }
        return sum;
    }

    static long throughArrays(int n) {
        Branch root = new Branch(0);
        Branch b = root;
        for (int i = 1; i < n; i++) {
            Branch child = new Branch(i % 10);
            b.children[i % 2] = child;
            b = child;
        }
        long sum = 0;
        Branch p = root;
        while (p != null) {
            sum += p.value;
            p = p.children[0] != null ? p.children[0] : p.children[1];
        }
        return sum;
    }

    static long throughInterface(int n) {
        Cell c = null;
        for (int i = 0; i < n; i++) {
            if (i % 2 == 0) {
                c = new Even(i % 10, c);
            } else {
                c = new Odd(i % 10, c);
            }
        }
        long sum = 0;
        for (Cell p = c; p != null; p = p.rest()) {
            sum += p.value();
        }
        return sum;
    }

    static long subclasses(int n) {
        Step head = null;
        for (int i = 0; i < n; i++) {
            Step s;
            if (i % 3 == 0) {
                Heavy h = new Heavy();
                h.extra = i % 5;
                s = h;
            } else {
                s = new Light();
            }
            s.value = i % 10;
            s.next = head;
            head = s;
        }
        long sum = 0;
        for (Step p = head; p != null; p = p.next) {
            sum += p.weight();
        }
        return sum;
    }

    static long generic(int n) {
        Generic<String> list = null;
        for (int i = 0; i < n; i++) {
            list = new Generic<>(i % 2 == 0 ? "a" : "bc", list);
        }
        long sum = 0;
        for (Generic<String> p = list; p != null; p = p.next) {
            sum += p.item.length();
        }
        return sum;
    }

    static long arrayOfLists(int lists, int n) {
        Node[] heads = new Node[lists];
        for (int i = 0; i < n; i++) {
            heads[i % lists] = new Node(i % 13, heads[i % lists]);
        }
        long sum = 0;
        for (Node head : heads) {
            for (Node p = head; p != null; p = p.next) {
                sum += p.value;
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println("singly linked " + singlyLinked(400000));
        System.out.println("frozen " + frozen(400000));
        System.out.println("doubly linked " + doublyLinked(300000));
        System.out.println("degenerate tree " + degenerateTree(300000));
        System.out.println("two classes " + twoClasses(300000));
        System.out.println("through arrays " + throughArrays(300000));
        System.out.println("through interface " + throughInterface(300000));
        System.out.println("subclasses " + subclasses(300000));
        System.out.println("generic " + generic(300000));
        System.out.println("array of lists " + arrayOfLists(3, 900000));
        long again = 0;
        for (int round = 0; round < 3; round++) {
            again += singlyLinked(300000 + round);
        }
        System.out.println("again " + again);
    }
}
