// `wait(millis, nanos)`: any nanoseconds lengthen the wait by a millisecond, and a negative
// timeout or nanoseconds out of range throw IllegalArgumentException with `Object`'s messages,
// checked before the monitor is.
public class WaitWithNanoseconds {
    static final Object lock = new Object();

    static void tryWait(long millis, int nanos) {
        try {
            synchronized (lock) {
                lock.wait(millis, nanos);
            }
            System.out.println("waited " + millis + " " + nanos);
        } catch (IllegalArgumentException | InterruptedException e) {
            System.out.println(millis + " " + nanos + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) throws InterruptedException {
        long start = System.nanoTime();
        synchronized (lock) {
            lock.wait(5, 1);
        }
        System.out.println("long enough: " + ((System.nanoTime() - start) >= 6_000_000L));
        tryWait(1, 0);
        tryWait(0, 999_999);
        tryWait(-1, 0);
        tryWait(-1, -1);
        tryWait(1, -1);
        tryWait(1, 1_000_000);
        try {
            lock.wait(1, 1);
        } catch (IllegalMonitorStateException e) {
            System.out.println("not held: " + e.getMessage());
        }
        try {
            lock.wait(1, 1_000_000);
        } catch (IllegalArgumentException e) {
            System.out.println("not held, out of range: " + e.getMessage());
        }
    }
}
