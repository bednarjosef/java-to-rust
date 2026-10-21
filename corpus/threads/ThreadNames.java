// The names, ids and states of threads: the main thread, threads numbered Thread-N, a named one
// and a renamed one, and a thread started twice.
public class ThreadNames {
    static class Named extends Thread {
        Named() {
        }

        Named(String name) {
            super(name);
        }

        @Override
        public void run() {
            System.out.println("running " + Thread.currentThread().getName());
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Thread main = Thread.currentThread();
        System.out.println(main.getName() + " " + main.getPriority() + " " + main.isDaemon());
        System.out.println(main);
        Named a = new Named();
        Named b = new Named("worker");
        Named c = new Named();
        System.out.println(a.getName() + " " + b.getName() + " " + c.getName());
        c.setName("renamed");
        a.start();
        a.join();
        b.start();
        b.join();
        c.start();
        c.join();
        System.out.println(a.isAlive() + " " + c.getName());
        System.out.println(a.getId() > main.getId());
        System.out.println(Thread.currentThread() == main);
        try {
            a.start();
        } catch (IllegalThreadStateException e) {
            System.out.println("started twice: " + e);
        }
    }
}
