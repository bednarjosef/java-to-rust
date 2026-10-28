import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

// Classes whose static initialization can fail, first used by several threads at once, in a
// program that catches the failures: one thread runs the initialization and the others wait for
// it, so each sees the value it sets, and where it fails, the thread that ran it gets
// `ExceptionInInitializerError` and every other `NoClassDefFoundError`.
public class ConcurrentClassInitialization {
    static class Slow {
        static final String VALUE;

        static {
            VALUE = compute(false);
        }
    }

    static class SlowBroken {
        static final String VALUE = compute(true);
    }

    static String compute(boolean fail) {
        try {
            Thread.sleep(100);
        } catch (InterruptedException e) {
            throw new IllegalStateException(e);
        }
        if (fail) {
            throw new IllegalStateException("no value");
        }
        return "ready";
    }

    public static void main(String[] args) throws InterruptedException {
        List<String> seen = Collections.synchronizedList(new ArrayList<>());
        List<Thread> threads = new ArrayList<>();
        for (int i = 0; i < 4; i++) {
            threads.add(new Thread(() -> seen.add(Slow.VALUE)));
        }
        for (int i = 0; i < 4; i++) {
            threads.add(new Thread(() -> {
                try {
                    seen.add(SlowBroken.VALUE);
                } catch (Throwable t) {
                    seen.add(t.getClass().getSimpleName());
                }
            }));
        }
        for (Thread t : threads) {
            t.start();
        }
        for (Thread t : threads) {
            t.join();
        }
        Collections.sort(seen);
        System.out.println(seen);
        try {
            System.out.println(SlowBroken.VALUE);
        } catch (NoClassDefFoundError e) {
            System.out.println(e.getMessage());
        }
    }
}
