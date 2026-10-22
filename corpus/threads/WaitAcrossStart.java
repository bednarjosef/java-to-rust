// `wait` and `notify` on a monitor that main entered before it started any thread: main waits in
// it for the worker it started from inside it, then the worker waits for main's answer.
public class WaitAcrossStart {
    static final Object lock = new Object();
    static int stage;
    static String answer;

    public static void main(String[] args) throws InterruptedException {
        Thread worker;
        synchronized (lock) {
            worker = new Thread(() -> {
                synchronized (lock) {
                    stage = 1;
                    lock.notifyAll();
                    while (answer == null) {
                        try {
                            lock.wait();
                        } catch (InterruptedException e) {
                            return;
                        }
                    }
                    stage = 3;
                    System.out.println("worker got " + answer);
                }
            });
            worker.start();
            while (stage != 1) {
                lock.wait();
            }
            System.out.println("main saw stage " + stage);
            answer = "go";
            stage = 2;
            lock.notifyAll();
        }
        worker.join();
        System.out.println("main ends at stage " + stage);
    }
}
