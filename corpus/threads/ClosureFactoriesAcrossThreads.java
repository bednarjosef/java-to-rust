// A static method that only returns a lambda of a closure type that a thread reaches, an `Arc`
// of a `dyn Fn + Send + Sync`: its calls in main are replaced by the lambda, stored for both
// threads and lent; the call on the other thread, whose argument is a capture of its lambda,
// stays a call.
public class ClosureFactoriesAcrossThreads {
    interface Op { int apply(int x); }

    static Op scaled(int k) { return x -> x * k; }

    static int sum(int n, Op op) {
        int total = 0;
        for (int i = 0; i < n; i++) total += op.apply(i);
        return total;
    }

    static Op kept;

    public static void main(String[] args) throws InterruptedException {
        int k = args.length + 3;
        kept = scaled(k);
        int[] out = new int[2];
        Thread t = new Thread(() -> {
            out[0] = kept.apply(2);
            out[1] = sum(4, scaled(k));
        });
        t.start();
        t.join();
        System.out.println(out[0] + " " + out[1] + " " + sum(3, scaled(k)) + " " + sum(2, kept));
    }
}
