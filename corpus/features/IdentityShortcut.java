/// The `a == b || .. a.equals(b) ..` shortcut with an assignment as an operand, as HashMap writes
/// it: the assignment happens whatever the identities say, and its variable is read afterwards.
public class IdentityShortcut {
    static class Node {
        Object key;
        Node next;

        Node(Object key, Node next) {
            this.key = key;
            this.next = next;
        }
    }

    // HashMap.getNode: the assigned operand on the left, the equals reading the variable.
    static String find(Node first, Object key) {
        Object k;
        for (Node e = first; e != null; e = e.next) {
            if ((k = e.key) == key || (key != null && key.equals(k))) {
                return "found " + k;
            }
        }
        return "absent " + key;
    }

    // The same test with the operands of == swapped.
    static boolean swapped(Object key, Object value) {
        Object k;
        return key == (k = value) || (key != null && key.equals(k));
    }

    // With the receiver and argument of equals swapped.
    static boolean reversed(Object key, Object value) {
        Object k;
        return (k = value) == key || (k != null && k.equals(key));
    }

    // The negated form, `a != b && .. !a.equals(b) ..`, with the assignment on the left.
    static boolean differs(Object key, Object value) {
        Object k;
        return (k = value) != key && (key == null || !key.equals(k));
    }

    // The negated form with the operands swapped and the equals reversed.
    static boolean differsSwapped(Object key, Object value) {
        Object k;
        return key != (k = value) && (k == null || !k.equals(key));
    }

    // The assigned variable is read after the test, whatever it said.
    static String last(Object key, Object... values) {
        Object k = "unset";
        boolean any = false;
        for (Object v : values) {
            any |= (k = v) == key || (key != null && key.equals(k));
        }
        return any + " " + k;
    }

    public static void main(String[] args) {
        String a = "apple";
        String same = new String("apple");
        String other = "pear";
        Integer big = Integer.valueOf(1000);
        Integer bigAgain = Integer.valueOf(1000);
        Integer small = 7;
        Long lng = 5000000000L;
        Long lngAgain = Long.valueOf(5000000000L);

        Node chain = new Node(other, new Node(null, new Node(big, new Node(a, null))));
        System.out.println(find(chain, same));
        System.out.println(find(chain, a));
        System.out.println(find(chain, bigAgain));
        System.out.println(find(chain, null));
        System.out.println(find(chain, "kiwi"));
        System.out.println(find(new Node(null, null), null));
        System.out.println(find(new Node(small, null), Integer.valueOf(7)));

        System.out.println(swapped(same, a) + " " + swapped(null, null) + " " + swapped(null, other));
        System.out.println(swapped(big, bigAgain) + " " + swapped(lng, lngAgain) + " " + swapped(small, "7"));
        System.out.println(reversed(a, same) + " " + reversed(big, bigAgain) + " " + reversed(other, a));
        System.out.println(differs(same, a) + " " + differs(null, null) + " " + differs(null, a));
        System.out.println(differs(lng, lngAgain) + " " + differsSwapped(small, 7) + " " + differsSwapped(a, other));
        System.out.println(differsSwapped(null, a) + " " + differsSwapped(a, null));

        System.out.println(last(a, same, other, null, a));
        System.out.println(last(null, "x", null, "y"));
        System.out.println(last("z", "z"));

        String seen = "";
        Object k = "none";
        for (Object v : new Object[] {a, same, big, bigAgain, null}) {
            if ((k = v) == a || (a != null && a.equals(k))) {
                seen += " hit " + k;
            } else {
                seen += " miss " + k;
            }
        }
        System.out.println(seen + " " + k);
    }
}
