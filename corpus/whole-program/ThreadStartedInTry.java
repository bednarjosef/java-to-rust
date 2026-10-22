// A thread started inside a `try` that catches what the start may throw, whose `run` throws an
// exception that a direct call of `run` catches: the exception that escapes the thread's `run`
// ends that thread (here through its handler), not the `try` around the start.
public class ThreadStartedInTry {
    public static void main(String[] args) throws InterruptedException {
        Runnable task = () -> {
            System.out.println("running");
            throw new IllegalStateException("boom");
        };
        try {
            new Thread(task).run();
        } catch (IllegalStateException e) {
            System.out.println("direct " + e.getMessage());
        }
        Thread t = new Thread(task);
        t.setUncaughtExceptionHandler((th, e) -> System.out.println("uncaught " + e.getMessage()));
        try {
            t.start();
        } catch (Throwable e) {
            System.out.println("caught " + e);
        }
        t.join();
        System.out.println("after");
    }
}
