// A NullPointerException from writing a field of an object that may be null, which a catch or a
// finally sees: an assignment, a compound assignment and an increment, as statements and as
// values.
public class NullFieldWritesCaught {
    static class Node {
        Node next;
        int count;
    }

    static Node find(int i) {
        return i > 0 ? new Node() : null;
    }

    static String update(int i) {
        Node n = find(i);
        try {
            n.next = null;
            n.count += 2;
            n.count++;
            int seen = n.count *= 3;
            return "count " + n.count + " seen " + seen;
        } catch (NullPointerException e) {
            return "no node";
        }
    }

    static int total(int i) {
        int done = 0;
        try {
            Node n = find(i);
            done = ++n.count;
        } finally {
            System.out.println("done " + done);
        }
        return done;
    }

    public static void main(String[] args) {
        System.out.println(update(1));
        System.out.println(update(0));
        System.out.println(total(1));
        try {
            System.out.println(total(0));
        } catch (NullPointerException e) {
            System.out.println("caught");
        }
    }
}
