// A worker's exception ends only the worker: it is printed as Java prints it, main joins the
// worker and goes on, and the program exits with 0. A runtime failure in another worker (a
// division by zero, an index out of bounds) ends that worker the same way.
public class UncaughtInThread {
    static int[] results = new int[3];

    static class Failing extends Thread {
        private final int kind;

        Failing(int kind) {
            super("worker-" + kind);
            this.kind = kind;
        }

        @Override
        public void run() {
            results[kind] = kind + 1;
            if (kind == 0) {
                throw new IllegalStateException("worker " + kind + " gave up");
            }
            if (kind == 1) {
                int zero = kind - 1;
                results[kind] = 10 / zero;
            }
            int[] small = new int[1];
            results[kind] = small[kind];
        }
    }

    public static void main(String[] args) throws InterruptedException {
        for (int k = 0; k < 3; k++) {
            Failing f = new Failing(k);
            f.start();
            f.join();
            System.out.println(f.getName() + " ended, alive " + f.isAlive() + ", result " + results[k]);
        }
        System.out.println("main goes on");
    }
}
