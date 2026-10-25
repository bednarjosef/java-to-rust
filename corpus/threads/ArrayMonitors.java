// Monitors of arrays: threads that share an array lock it around their updates, a thread waits
// on an array until another notifies it, holdsLock tells whether the thread holds an array's
// monitor, and an array that only one thread uses is locked as well.
public class ArrayMonitors {
    static final long[] totals = new long[2];
    static final Object[] slots = new Object[1];

    public static void main(String[] args) throws InterruptedException {
        Thread[] workers = new Thread[4];
        for (int i = 0; i < workers.length; i++) {
            int k = i;
            workers[i] = new Thread(() -> {
                for (int n = 0; n < 10_000; n++) {
                    synchronized (totals) {
                        totals[0] += k;
                        totals[1]++;
                    }
                }
            });
            workers[i].start();
        }
        for (Thread w : workers) {
            w.join();
        }
        System.out.println("totals: " + totals[0] + " " + totals[1]);

        Thread waiter = new Thread(() -> {
            synchronized (slots) {
                while (slots[0] == null) {
                    try {
                        slots.wait();
                    } catch (InterruptedException e) {
                        return;
                    }
                }
                System.out.println("got: " + slots[0] + " " + Thread.holdsLock(slots));
            }
        });
        waiter.start();
        synchronized (slots) {
            slots[0] = "ready";
            slots.notifyAll();
        }
        waiter.join();
        System.out.println("after: " + Thread.holdsLock(slots));

        int[] mine = {1, 2, 3};
        synchronized (mine) {
            mine[0] = Thread.holdsLock(mine) ? 10 : -1;
        }
        System.out.println("mine: " + mine[0] + " " + Thread.holdsLock(mine));

        int[] missing = args.length > 5 ? mine : null;
        try {
            synchronized (missing) {
                System.out.println("unreachable");
            }
        } catch (NullPointerException e) {
            System.out.println("null lock");
        }
    }
}
