// Five philosophers share five forks, plain objects used only as locks. Each picks up the fork
// with the lower number first, so no cycle of waits forms, and eats a fixed number of meals.
public class DiningPhilosophers {
    static final int MEALS = 2000;
    static int served;

    static class Philosopher extends Thread {
        private final Object left;
        private final Object right;
        private final int seat;
        int meals;

        Philosopher(int seat, Object left, Object right) {
            this.seat = seat;
            this.left = left;
            this.right = right;
        }

        @Override
        public void run() {
            for (int i = 0; i < MEALS; i++) {
                synchronized (left) {
                    synchronized (right) {
                        meals++;
                        synchronized (DiningPhilosophers.class) {
                            served++;
                        }
                    }
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Object[] forks = new Object[5];
        for (int i = 0; i < forks.length; i++) {
            forks[i] = new Object();
        }
        Philosopher[] table = new Philosopher[5];
        for (int i = 0; i < table.length; i++) {
            int a = i;
            int b = (i + 1) % forks.length;
            table[i] = new Philosopher(i, forks[Math.min(a, b)], forks[Math.max(a, b)]);
        }
        for (Philosopher p : table) {
            p.start();
        }
        for (Philosopher p : table) {
            p.join();
            System.out.println("philosopher " + p.seat + " ate " + p.meals + " meals");
        }
        System.out.println("served " + served);
    }
}
