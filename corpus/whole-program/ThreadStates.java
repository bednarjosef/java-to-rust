// `Thread.getState()` as Java reports it: a thread is NEW until it starts, TIMED_WAITING while it
// sleeps, WAITING in `wait` and `join`, BLOCKED while another thread holds the monitor it enters,
// RUNNABLE while it runs, and TERMINATED once it has ended. Each state is waited for (a thread
// gets there in its own time), and nothing ends before the main thread lets it. Then the context
// class loader: the main thread's is the application's, a new thread starts with its parent's,
// and one that was set to null stays null.
public class ThreadStates {
    static final Object lock = new Object();
    static boolean released;

    static Thread.State reach(Thread t, Thread.State wanted) throws InterruptedException {
        for (int i = 0; i < 2000 && t.getState() != wanted; i++) {
            Thread.sleep(1);
        }
        return t.getState();
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println("main: " + Thread.currentThread().getState());
        Thread sleeper = new Thread(() -> {
            try {
                Thread.sleep(60_000);
            } catch (InterruptedException e) {
                System.out.println("sleeper interrupted");
            }
        });
        System.out.println("before start: " + sleeper.getState());
        sleeper.start();
        System.out.println("sleeping: " + reach(sleeper, Thread.State.TIMED_WAITING));

        Thread waiter = new Thread(() -> {
            synchronized (lock) {
                while (!released) {
                    try {
                        lock.wait();
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
        });
        waiter.start();
        System.out.println("in wait: " + reach(waiter, Thread.State.WAITING));

        Thread joiner = new Thread(() -> {
            try {
                sleeper.join();
            } catch (InterruptedException e) {
                return;
            }
        });
        joiner.start();
        System.out.println("in join: " + reach(joiner, Thread.State.WAITING));

        Thread entering;
        synchronized (lock) {
            entering = new Thread(() -> {
                synchronized (lock) {
                    released = true;
                }
            });
            entering.start();
            System.out.println("entering a held monitor: " + reach(entering, Thread.State.BLOCKED));
            released = true;
            lock.notifyAll();
        }
        sleeper.interrupt();
        for (Thread t : new Thread[] {sleeper, waiter, joiner, entering}) {
            t.join();
        }
        System.out.println("ended: " + sleeper.getState() + " " + waiter.getState() + " " + joiner.getState()
                + " " + entering.getState());

        ClassLoader app = Thread.currentThread().getContextClassLoader();
        System.out.println("main's loader: " + (app != null) + " " + (app == ClassLoader.getSystemClassLoader()));
        Thread child = new Thread(() -> {
            Thread me = Thread.currentThread();
            System.out.println("child: " + me.getState() + " " + (me.getContextClassLoader() == app));
            me.setContextClassLoader(null);
            Thread grandchild = new Thread(() -> System.out.println(
                    "grandchild's loader: " + Thread.currentThread().getContextClassLoader()));
            grandchild.start();
            try {
                grandchild.join();
            } catch (InterruptedException e) {
                return;
            }
        });
        child.start();
        child.join();
    }
}
