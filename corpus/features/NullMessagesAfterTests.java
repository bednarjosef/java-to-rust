// The messages of NullPointerExceptions that a catch reads, where tests and assignments show
// that some dereferences never fail: a variable tested as it is assigned
// (`(k = e.key()) != null && k.length() > 0`), one set when it was null (`if (x == null) x = ..`),
// and a static that its class's initializer sets first. The others fail with Java's messages:
// through an assignment (`(s = a).next`), and of an explicit unboxing.
public class NullMessagesAfterTests {
    static final class Node {
        Node next;
        String key;

        Node(String key) {
            this.key = key;
        }

        String key() {
            return key;
        }
    }

    static final Node FIRST;
    static int created;

    static {
        FIRST = new Node("first");
        created = 1;
    }

    static int keyLength(Node e) {
        String k;
        return e != null && (k = e.key()) != null && k.length() > 0 ? k.length() : -1;
    }

    static String orDefault(String x) {
        if (x == null) {
            x = "default";
        }
        return x.toUpperCase();
    }

    static Node nextOf(Node a) {
        Node s;
        return (s = a).next;
    }

    static int unboxed(Integer i) {
        return i.intValue();
    }

    public static void main(String[] args) {
        System.out.println(keyLength(new Node("abc")) + " " + keyLength(new Node(null)) + " " + keyLength(null));
        System.out.println(orDefault(null) + " " + orDefault("given"));
        System.out.println(FIRST.key() + " " + created);
        try {
            nextOf(null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            unboxed(null);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(nextOf(FIRST) == null);
    }
}
