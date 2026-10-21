// The messages of the NullPointerExceptions that the JVM raises on locals, which a class file
// compiled without -g names by their slots (<localN>): after long and double locals (two slots
// each), in a slot that a block's local left free, after the hidden locals that javac adds for a
// for-each loop over an array, a switch on a string and a try with a finally, in a lambda's
// body, whose captured locals are the first parameters of its method, and in a local or
// anonymous class, which holds them in fields of its own (this.val$name).
public class LocalNullPointerMessages {
    static class Node {
        String name;
        Node next;

        Node(String name) {
            this.name = name;
        }
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static Node nothing() {
        return null;
    }

    static void wide(int count) {
        long total = count;
        double average = total / 2.0;
        Node node = nothing();
        try {
            System.out.println(node.name + total + average);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void reused() {
        {
            Node first = new Node("first");
            System.out.println(first.name);
        }
        Node second = nothing();
        try {
            System.out.println(second.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void arrays(Node[] nodes) {
        for (Node node : nodes) {
            try {
                System.out.println(node.name.length());
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
        Node after = nothing();
        try {
            System.out.println(after.next);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void strings(String key, String text) {
        switch (key) {
            case "a":
                System.out.println("a");
                break;
            default:
                System.out.println("other");
        }
        Node after = nothing();
        try {
            System.out.println(after.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            switch (text) {
                case "b":
                    System.out.println("b");
                    break;
                default:
                    System.out.println("other");
            }
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void finished(Node start) {
        try {
            System.out.println(start.name);
        } finally {
            System.out.println("finally");
        }
        Node after = start.next;
        try {
            System.out.println(after.name);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    static void caught(Node start) {
        try {
            System.out.println(start.next.name);
        } catch (NullPointerException e) {
            Node inside = start.next;
            try {
                System.out.println(inside.name);
            } catch (NullPointerException again) {
                System.out.println(again.getMessage());
            }
        }
    }

    static void lambdas(Node start, String label) {
        Node missing = start.next;
        report(() -> System.out.println(missing.name));
        report(() -> System.out.println(label + missing.next));
        report(() -> {
            Node own = missing;
            System.out.println(label + own.name);
        });
    }

    static void classes(Node start) {
        Node missing = start.next;
        class Reader implements Action {
            public void run() {
                System.out.println(missing.name);
            }
        }
        report(new Reader());
        report(new Action() {
            public void run() {
                System.out.println(missing.next);
            }
        });
    }

    Node held;

    void instanceLambda() {
        report(() -> System.out.println(held.name));
    }

    public static void main(String[] args) {
        wide(3);
        reused();
        arrays(new Node[] {new Node("ab"), new Node(null), null});
        strings("a", null);
        finished(new Node("d"));
        caught(new Node("e"));
        lambdas(new Node("f"), "label ");
        new LocalNullPointerMessages().instanceLambda();
        classes(new Node("g"));
    }
}
