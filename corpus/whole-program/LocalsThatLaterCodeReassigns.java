// Locals that code written after a null test or after a borrow assigns, as the JDK's
// `ConcurrentLinkedDeque` iterator (`for (;; p = nextNode(p)) { if (p == null) break; .. }`) and
// `CompletableFuture.cleanStack` (`casStack(p, p = p.next)`) do.
public class LocalsThatLaterCodeReassigns {
    static final class Node {
        final int value;
        Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    Node head;
    Node found;
    int item;

    Node nextNode(Node p) {
        return p.next;
    }

    // A `for` whose update assigns the local that its body tests for null and then reads.
    void advance(int wanted) {
        Node p = head;
        for (;; p = nextNode(p)) {
            if (p == null) {
                found = null;
                item = -1;
                break;
            }
            if (p.value >= wanted) {
                found = p;
                item = p.value;
                break;
            }
        }
    }

    boolean casHead(Node expected, Node next) {
        if (head != expected) {
            return false;
        }
        head = next;
        return true;
    }

    // An argument that is a local, and a later argument that assigns it: the call gets the old
    // value.
    int unlinkAll() {
        Node p = head;
        int unlinked = 0;
        while (p != null) {
            if (casHead(p, p = p.next)) {
                unlinked++;
            } else {
                p = head;
            }
        }
        return unlinked;
    }

    public static void main(String[] args) {
        LocalsThatLaterCodeReassigns list = new LocalsThatLaterCodeReassigns();
        list.head = new Node(1, new Node(3, new Node(5, null)));
        for (int wanted : new int[] {0, 3, 4, 9}) {
            list.advance(wanted);
            System.out.println(wanted + " -> " + list.item + " " + (list.found != null));
        }
        System.out.println(list.unlinkAll() + " " + (list.head == null));
    }
}
