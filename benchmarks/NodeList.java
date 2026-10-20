// A hand-written singly linked list: allocation of a node per element, pointer chasing, and
// mutation of the fields of the nodes.
public class NodeList {
    static class Node {
        int value;
        Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    Node head;
    int size;

    void pushFront(int value) {
        head = new Node(value, head);
        size++;
    }

    void reverse() {
        Node previous = null;
        Node current = head;
        while (current != null) {
            Node next = current.next;
            current.next = previous;
            previous = current;
            current = next;
        }
        head = previous;
    }

    void removeMultiplesOf(int k) {
        Node previous = null;
        Node current = head;
        while (current != null) {
            if (current.value % k == 0) {
                if (previous == null) {
                    head = current.next;
                } else {
                    previous.next = current.next;
                }
                size--;
            } else {
                previous = current;
            }
            current = current.next;
        }
    }

    long sum() {
        long total = 0;
        for (Node n = head; n != null; n = n.next) {
            total += n.value;
        }
        return total;
    }

    static long run(int n, int rounds) {
        long checksum = 0;
        for (int round = 0; round < rounds; round++) {
            NodeList list = new NodeList();
            int x = round + 1;
            for (int i = 0; i < n; i++) {
                x = x * 1103515245 + 12345;
                list.pushFront((x >>> 8) % 1000);
            }
            list.reverse();
            list.removeMultiplesOf(3);
            list.reverse();
            checksum = checksum * 31 + list.sum() + list.size;
        }
        return checksum;
    }

    public static void main(String[] args) {
        System.out.println("checksum " + run(1000000, 12));
    }
}
