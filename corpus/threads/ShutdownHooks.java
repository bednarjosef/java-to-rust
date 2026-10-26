public class ShutdownHooks {
    static int saved;

    public static void main(String[] args) throws Exception {
        Thread save = new Thread("saver") {
            @Override
            public void run() {
                System.out.println("hook " + getName() + " saves " + saved);
            }
        };
        Runtime.getRuntime().addShutdownHook(save);
        Thread dropped = new Thread(() -> System.out.println("never runs"));
        Runtime.getRuntime().addShutdownHook(dropped);
        System.out.println("removed: " + Runtime.getRuntime().removeShutdownHook(dropped));
        System.out.println("removed again: " + Runtime.getRuntime().removeShutdownHook(dropped));
        try {
            Runtime.getRuntime().addShutdownHook(save);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        try {
            Runtime.getRuntime().removeShutdownHook(null);
        } catch (NullPointerException e) {
            System.out.println("remove(null): " + e.getClass().getName());
        }
        Thread running = new Thread(() -> {});
        running.start();
        running.join();
        Thread worker = new Thread(() -> {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                throw new RuntimeException(e);
            }
            saved = 42;
            System.out.println("worker done");
        });
        worker.start();
        System.out.println("main returns");
    }
}
