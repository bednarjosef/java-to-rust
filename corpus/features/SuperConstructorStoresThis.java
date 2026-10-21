// A superclass's constructor stores `this` in another object and in a field of its own: the
// object stored is the whole object being built, which later code reaches as its subclass.
public class SuperConstructorStoresThis {
    static class Registry {
        Node first;
        int count;

        void add(Node node) {
            node.nextInRegistry = first;
            first = node;
            count++;
        }
    }

    abstract static class Node {
        Node self;
        Node nextInRegistry;
        Node previous;
        int id;

        Node(Registry registry, Node previous) {
            self = this;
            this.previous = previous;
            if (previous != null) {
                previous.link(this);
            }
            registry.add(this);
            id = registry.count;
        }

        void link(Node next) {
            System.out.println(describe() + " -> " + next.id);
        }

        abstract String describe();
    }

    static class Leaf extends Node {
        String label;

        Leaf(Registry registry, Node previous, String label) {
            super(registry, previous);
            this.label = label;
        }

        String describe() {
            return "leaf " + label + "#" + id;
        }
    }

    static class Pair extends Node {
        int weight = 10;

        Pair(Registry registry, Node previous) {
            super(registry, previous);
            weight += id;
        }

        String describe() {
            return "pair w" + weight + "#" + id;
        }
    }

    public static void main(String[] args) {
        Registry registry = new Registry();
        Node a = new Leaf(registry, null, "a");
        Node b = new Pair(registry, a);
        Node c = new Leaf(registry, b, "c");
        System.out.println(a.self == a);
        System.out.println(b.self == b);
        System.out.println(c.previous == b);
        for (Node n = registry.first; n != null; n = n.nextInRegistry) {
            System.out.println(n.describe() + " self " + (n.self == n));
        }
        System.out.println(registry.count);
    }
}
