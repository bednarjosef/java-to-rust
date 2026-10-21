// A worker builds a long linked list that main reads after join; dropping it must not overflow.
public class SharedChain {
    static class Node {
        final int value;
        Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    static class Builder extends Thread {
        Node head;

        @Override
        public void run() {
            for (int i = 0; i < 200000; i++) {
                head = new Node(i, head);
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Builder b = new Builder();
        b.start();
        b.join();
        long sum = 0;
        int count = 0;
        for (Node n = b.head; n != null; n = n.next) {
            sum += n.value;
            count++;
        }
        System.out.println(count + " " + sum);
        b.head = null;
        System.out.println("dropped");
    }
}
