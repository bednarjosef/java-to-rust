// A daemon thread that never ends does not keep the program alive, and a thread that is not a
// daemon does: main returns first, and the program ends once the worker is done.
public class DaemonExit {
    static class Forever extends Thread {
        @Override
        public void run() {
            while (true) {
                try {
                    Thread.sleep(1000);
                } catch (InterruptedException e) {
                    return;
                }
            }
        }
    }

    static class Last extends Thread {
        @Override
        public void run() {
            try {
                Thread.sleep(100);
            } catch (InterruptedException e) {
                return;
            }
            System.out.println("the last thread ends after main");
        }
    }

    public static void main(String[] args) {
        Forever daemon = new Forever();
        daemon.setDaemon(true);
        System.out.println("daemon: " + daemon.isDaemon());
        daemon.start();
        try {
            daemon.setDaemon(false);
        } catch (IllegalThreadStateException e) {
            System.out.println("setDaemon on a live thread: " + e);
        }
        new Last().start();
        System.out.println("main returns");
    }
}
