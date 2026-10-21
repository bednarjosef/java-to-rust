// The messages of the NullPointerExceptions that the JVM raises, read by a catch: what failed,
// and where the null came from, named as in a class file compiled without -g (parameters as
// <parameterN>, fields through them, static fields, the values that methods return, elements).
public class RuntimeNullPointerMessages {
    static class Node {
        String name;
        Node next;
        int[] counts;
        Node[] children;
        int size;
        static Node shared;

        Node(String name) {
            this.name = name;
        }

        Node following() {
            return next;
        }

        int nameLength() {
            return next.name.length();
        }

        void rename(String name) {
            this.name = name;
        }
    }

    static Node find(String name) {
        return null;
    }

    static void invoke(Node node) {
        try {
            node.rename("x");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void fields(Node node, long weight, Node other) {
        try {
            System.out.println(other.next.name.length());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            other.next.size = 3;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            other.next.size += 3;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            other.next.size++;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(other.next.next.next.next.next.next.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void arrays(Node node, int i) {
        try {
            System.out.println(node.counts[0]);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            node.counts[i] = 4;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.counts.length);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            node.children[1] = node;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        Node[] children = new Node[3];
        node.children = children;
        try {
            System.out.println(node.children[i].name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.children[2].name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.children[i + 1].name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void calls(Node node, boolean pick) {
        try {
            System.out.println(find("a").name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.following().name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.nameLength());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(Node.shared.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println((pick ? node : node.next).name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void strings(String text, Node node) {
        try {
            System.out.println(text.length());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.name.equals("a"));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(node.name.charAt(0));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void statements(Object lock, RuntimeException failure) {
        try {
            synchronized (lock) {
                System.out.println("locked");
            }
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            throw failure;
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    Node own;

    void ownField() {
        try {
            System.out.println(own.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(this.own.next);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        invoke(null);
        fields(null, 2, new Node("a"));
        arrays(new Node("b"), 0);
        calls(new Node("c"), false);
        strings(null, new Node(null));
        statements(null, null);
        new RuntimeNullPointerMessages().ownField();
        Node chain = new Node("d");
        chain.next = new Node("e");
        chain.next.next = new Node("f");
        chain.next.next.next = new Node("g");
        chain.next.next.next.next = new Node("h");
        chain.next.next.next.next.next = new Node("i");
        fields(null, 2, chain);
    }
}
