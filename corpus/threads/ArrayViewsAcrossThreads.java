// Counting loops over arrays that threads share, each thread reading and writing its own elements
// of the same arrays through local variables while the others run, and reading the results after
// they end.
public class ArrayViewsAcrossThreads {
    static final int[] squares = new int[1000];
    static final String[] labels = new String[1000];

    static void fill(int[] into, String[] named, int k, int threads) {
        for (int round = 0; round < 50; round++) {
            for (int i = 0; i < into.length; i++) {
                if (i % threads == k) {
                    into[i] = i * i + round;
                    named[i] = "t" + k + ":" + into[i];
                }
            }
        }
    }

    static long total(int[] values) {
        long sum = 0;
        for (int i = 0; i < values.length; i++) {
            sum += values[i];
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int threads = 4;
        Thread[] workers = new Thread[threads];
        for (int k = 0; k < threads; k++) {
            int mine = k;
            workers[k] = new Thread(() -> fill(squares, labels, mine, threads));
            workers[k].start();
        }
        for (Thread w : workers) {
            w.join();
        }
        System.out.println("total " + total(squares));
        System.out.println(labels[0] + " " + labels[1] + " " + labels[998] + " " + labels[999]);
    }
}
