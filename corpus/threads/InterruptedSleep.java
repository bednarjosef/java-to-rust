// interrupt ends a sleep and a join with InterruptedException, which clears the status; the
// status is kept before a thread starts and after it ends; Thread.interrupted() reads and clears.
public class InterruptedSleep {
    static String outcome = "not run";

    static class Spinner extends Thread {
        long turns;

        @Override
        public void run() {
            while (!isInterrupted()) {
                turns++;
            }
            outcome = "spinner stopped, still interrupted: " + isInterrupted();
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Thread sleeper = new Thread(() -> {
            try {
                Thread.sleep(60000);
                outcome = "slept";
            } catch (InterruptedException e) {
                outcome = e.getMessage() + ", still interrupted: " + Thread.currentThread().isInterrupted();
            }
        });
        sleeper.start();
        sleeper.interrupt();
        sleeper.join();
        System.out.println(outcome);

        Thread early = new Thread(() -> outcome = "interrupted at the start: " + Thread.interrupted());
        early.interrupt();
        System.out.println("before the start: " + early.isInterrupted());
        early.start();
        early.join();
        System.out.println(outcome);
        System.out.println("after the end: " + early.isInterrupted());

        Thread self = new Thread(() -> Thread.currentThread().interrupt());
        self.start();
        self.join();
        System.out.println("interrupted itself: " + self.isInterrupted());

        Thread.currentThread().interrupt();
        System.out.println("main interrupted: " + Thread.currentThread().isInterrupted());
        try {
            Thread.sleep(10);
            System.out.println("slept anyway");
        } catch (InterruptedException e) {
            System.out.println("main: " + e);
        }
        System.out.println("status after the exception: " + Thread.interrupted());
        Thread.currentThread().interrupt();
        System.out.println("interrupted twice: " + Thread.interrupted() + " " + Thread.interrupted());

        Thread.currentThread().interrupt();
        try {
            Thread.sleep(-1);
        } catch (IllegalArgumentException e) {
            System.out.println("negative first: " + e.getMessage() + ", " + Thread.interrupted());
        }

        Thread.currentThread().interrupt();
        self.join();
        System.out.println("join of an ended thread returns, " + Thread.interrupted());

        Thread longer = new Thread(() -> {
            try {
                Thread.sleep(60000);
            } catch (InterruptedException e) {
                outcome = "longer: " + e.getMessage();
            }
        });
        longer.start();
        Thread.currentThread().interrupt();
        try {
            longer.join();
            System.out.println("joined");
        } catch (InterruptedException e) {
            System.out.println("join: " + e + ", message " + e.getMessage());
        }
        longer.interrupt();
        longer.join();
        System.out.println(outcome);

        Thread main = Thread.currentThread();
        Thread waker = new Thread(() -> {
            try {
                Thread.sleep(20);
            } catch (InterruptedException e) {
                return;
            }
            main.interrupt();
        });
        waker.start();
        try {
            Thread.sleep(60000);
            System.out.println("not woken");
        } catch (InterruptedException e) {
            System.out.println("woken: " + e.getMessage());
        }
        waker.join();

        Spinner spinner = new Spinner();
        spinner.start();
        Thread.sleep(5);
        spinner.interrupt();
        spinner.join();
        System.out.println(outcome);
        System.out.println("turns counted: " + (spinner.turns >= 0));
    }
}
