// A closure type that a thread reaches is an `Arc` of a `dyn Fn + Send + Sync`. A parameter that
// only calls such a closure, and lends it on, is lent as a `&dyn Fn` (an `Option` of one where it
// may be null): a stored closure, one that may be null, is lent as a borrow of what its handle
// holds, on the main thread and on another.
public class LentClosuresAcrossThreads {
    interface Op { int apply(int x); }

    static Op kept;

    static int run(Op op, int x) { return op.apply(x) + 1; }

    static int twice(Op op, int x) { return run(op, run(op, x)); }

    public static void main(String[] args) throws InterruptedException {
        int k = args.length + 3;
        kept = x -> x * k;
        int[] out = new int[2];
        Thread t = new Thread(() -> {
            out[0] = run(kept, 5);
            out[1] = twice(kept, 1);
        });
        t.start();
        t.join();
        System.out.println("thread " + out[0] + " " + out[1]);
        System.out.println("main " + run(kept, 2) + " " + twice(kept, 2) + " " + run(x -> x - k, 10));
        kept = null;
        try {
            System.out.println(run(kept, 1));
        } catch (NullPointerException e) {
            System.out.println("caught a null op");
        }
    }
}
