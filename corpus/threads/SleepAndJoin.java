// A worker that sleeps before it records its result: main joins with a timeout first, which
// returns while the worker still sleeps, then joins for good.
public class SleepAndJoin {
    static class Sleeper extends Thread {
        private final long millis;
        private int steps;

        Sleeper(long millis) {
            this.millis = millis;
        }

        @Override
        public void run() {
            for (int i = 0; i < 3; i++) {
                try {
                    Thread.sleep(millis);
                } catch (InterruptedException e) {
                    return;
                }
                steps++;
            }
        }

        int steps() {
            return steps;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Sleeper slow = new Sleeper(200);
        slow.start();
        slow.join(10);
        System.out.println("after a short join, alive: " + slow.isAlive());
        slow.join();
        System.out.println("after join, alive: " + slow.isAlive() + ", steps: " + slow.steps());
        Sleeper[] quick = {new Sleeper(1), new Sleeper(2), new Sleeper(3)};
        for (Sleeper s : quick) {
            s.start();
        }
        int total = 0;
        for (Sleeper s : quick) {
            s.join();
            total += s.steps();
        }
        System.out.println("total steps: " + total);
    }
}
