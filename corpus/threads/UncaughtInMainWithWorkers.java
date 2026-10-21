// main throws while a worker still runs: the exception is printed at once, the worker finishes
// its output, and only then does the program exit, with 1.
public class UncaughtInMainWithWorkers {
    static final Object gate = new Object();
    static boolean thrown;

    static class Worker extends Thread {
        @Override
        public void run() {
            synchronized (gate) {
                while (!thrown) {
                    try {
                        gate.wait(5);
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
            for (int i = 0; i < 3; i++) {
                System.out.println("worker line " + i);
            }
        }
    }

    static void fail() {
        synchronized (gate) {
            thrown = true;
            gate.notifyAll();
        }
        throw new IllegalArgumentException("main failed");
    }

    public static void main(String[] args) {
        Worker w = new Worker();
        w.start();
        System.out.println("main started the worker");
        fail();
    }
}
