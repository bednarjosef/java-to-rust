// Builders that several threads reach: appended to under their own monitor, kept as an
// `Object`, held in an array and a field that may be null, and captured by a worker's lambda.
public class SharedBuilders {
    static final Object lock = new Object();
    static StringBuilder log = new StringBuilder();
    static StringBuilder maybe;

    static class Writer extends Thread {
        final StringBuilder out;
        final char mark;

        Writer(StringBuilder out, char mark) {
            this.out = out;
            this.mark = mark;
        }

        @Override
        public void run() {
            for (int i = 0; i < 1000; i++) {
                synchronized (out) {
                    out.append(mark);
                }
            }
        }
    }

    static String describe(Object o) {
        return o == null ? "null" : o.toString();
    }

    public static void main(String[] args) throws InterruptedException {
        StringBuilder shared = new StringBuilder();
        Writer a = new Writer(shared, 'a');
        Writer b = new Writer(shared, 'b');
        a.start();
        b.start();
        a.join();
        b.join();
        int as = 0;
        for (int i = 0; i < shared.length(); i++) {
            if (shared.charAt(i) == 'a') {
                as++;
            }
        }
        System.out.println(shared.length() + " " + as);
        synchronized (shared) {
            shared.notifyAll();
            System.out.println(Thread.holdsLock(shared) + " " + Thread.holdsLock(log));
        }

        Object held = log;
        synchronized (lock) {
            log.append("seen");
        }
        System.out.println(describe(held) + " " + (held == log) + " " + held.equals(log));
        System.out.println(describe(maybe));

        StringBuilder[] parts = new StringBuilder[3];
        for (int i = 0; i < parts.length; i++) {
            parts[i] = new StringBuilder().append(i);
        }
        Thread worker = new Thread(() -> {
            for (StringBuilder p : parts) {
                p.append('!');
            }
            maybe = parts[1];
        });
        worker.start();
        worker.join();
        System.out.println(parts[0] + " " + parts[1] + " " + parts[2] + " " + describe(maybe));
        System.out.println(maybe == parts[1]);
    }
}
