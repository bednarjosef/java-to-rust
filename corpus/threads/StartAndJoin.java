// Workers that subclass Thread sum ranges into their own fields; main starts them all, joins
// them, and reads the fields.
public class StartAndJoin {
    static class Worker extends Thread {
        private final int from;
        private final int to;
        long sum;

        Worker(int from, int to) {
            this.from = from;
            this.to = to;
        }

        @Override
        public void run() {
            long s = 0;
            for (int i = from; i < to; i++) {
                s += (long) i * i;
            }
            sum = s;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Worker[] workers = new Worker[4];
        for (int i = 0; i < workers.length; i++) {
            workers[i] = new Worker(i * 1000, (i + 1) * 1000);
            workers[i].start();
        }
        long total = 0;
        for (Worker w : workers) {
            w.join();
            total += w.sum;
        }
        System.out.println("total " + total);
        System.out.println(workers[0].getName() + " alive " + workers[0].isAlive());
    }
}
