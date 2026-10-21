// Uncaught-exception handlers: a thread's own handler, the thread group's (which a subclass may
// override), the default handler, a failure of the runtime handed to a handler, an exception
// that a handler lets escape, and main's own uncaught exception going to the default handler.
public class UncaughtHandlers {
    static class Failing extends Thread {
        Failing(ThreadGroup group, String name) {
            super(group, name);
        }

        @Override
        public void run() {
            throw new IllegalStateException("failed in " + getName());
        }
    }

    static class Logger implements Thread.UncaughtExceptionHandler {
        int seen;

        @Override
        public void uncaughtException(Thread t, Throwable e) {
            seen++;
            System.out.println("logger: " + t.getName() + " threw " + e);
        }
    }

    static class Quiet extends ThreadGroup {
        Quiet(String name) {
            super(name);
        }

        @Override
        public void uncaughtException(Thread t, Throwable e) {
            System.out.println("group " + getName() + " got " + e.getMessage());
        }
    }

    // A thread's id depends on the threads that the JVM starts for itself.
    static String withoutId(Thread t) {
        String s = t.toString();
        return s.substring(s.indexOf(','));
    }

    public static void main(String[] args) throws InterruptedException {
        Thread plain = new Thread(() -> {
            throw new IllegalArgumentException("no handler");
        });
        System.out.println("handler is the group: " + (plain.getUncaughtExceptionHandler() == plain.getThreadGroup()));
        System.out.println("default: " + Thread.getDefaultUncaughtExceptionHandler());
        Logger logger = new Logger();
        Thread own = new Thread(() -> {
            throw new UnsupportedOperationException("own handler");
        }, "own");
        own.setUncaughtExceptionHandler(logger);
        System.out.println("own handler set: " + (own.getUncaughtExceptionHandler() == logger));
        own.start();
        own.join();
        System.out.println("after the end: " + own.getUncaughtExceptionHandler());

        plain.start();
        plain.join();

        ThreadGroup workers = new ThreadGroup("workers");
        System.out.println(workers + " in " + workers.getParent().getName());
        Thread grouped = new Thread(workers, () -> {
            Thread inner = new Thread(() -> { });
            System.out.println("inner thread's group: " + inner.getThreadGroup().getName());
        }, "grouped");
        System.out.println(withoutId(grouped) + " " + grouped.getThreadGroup().getName());
        grouped.start();
        grouped.join();
        System.out.println("ended: " + grouped.getThreadGroup() + " " + withoutId(grouped));

        Failing quiet = new Failing(new Quiet("quiet"), "q");
        quiet.start();
        quiet.join();

        Thread.setDefaultUncaughtExceptionHandler(logger);
        int[] numbers = new int[2];
        Thread index = new Thread(() -> System.out.println(numbers[numbers.length]), "index");
        index.start();
        index.join();
        Failing failing = new Failing(workers, "w");
        failing.start();
        failing.join();
        System.out.println("logger saw " + logger.seen);

        Thread.setDefaultUncaughtExceptionHandler((t, e) -> {
            throw new IllegalStateException("handler failed");
        });
        Thread broken = new Thread(() -> {
            throw new RuntimeException("for a broken handler");
        }, "broken");
        broken.start();
        broken.join();
        System.out.println("after the broken handler");

        Thread.setDefaultUncaughtExceptionHandler((t, e) -> System.out.println("default for " + t.getName() + ": " + e.getMessage()));
        throw new IllegalStateException("main ends");
    }
}
