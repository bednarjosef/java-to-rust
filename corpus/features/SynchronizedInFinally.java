// A `synchronized` block inside a `try` with a `finally`, as `ReferenceQueue.poll` has: the
// lock is a final field that is never null, and the `finally` runs after the block's `return`.
public class SynchronizedInFinally {
    private static class Lock { }

    private final Lock lock = new Lock();
    private String head = "first";

    String take() {
        String taken = head;
        head = null;
        return taken;
    }

    String poll() {
        if (head == null) {
            return null;
        }
        try {
            synchronized (lock) {
                return take();
            }
        } finally {
            System.out.println("unlocked");
        }
    }

    public static void main(String[] args) {
        SynchronizedInFinally queue = new SynchronizedInFinally();
        System.out.println(queue.poll());
        System.out.println(queue.poll());
    }
}
