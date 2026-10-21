// System.exit from a worker ends the whole program at once with its status, while main waits in
// join; the output printed before it is kept.
public class ExitFromWorker {
    static class Quitter extends Thread {
        @Override
        public void run() {
            System.out.println("worker exits");
            System.exit(3);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println("main starts the worker");
        Quitter q = new Quitter();
        q.start();
        q.join();
        System.out.println("never printed");
    }
}
