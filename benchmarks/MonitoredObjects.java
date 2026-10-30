// Many small objects in a program that waits and notifies, of a class whose objects it locks, so
// that every one keeps its monitor: 3 million points of one `long` each, all alive at once.
// What it measures is the memory they take (`scripts/benchmark.py` reports the peak).
public class MonitoredObjects {
    static class Point {
        long x;

        Point(long x) {
            this.x = x;
        }
    }

    static final Object lock = new Object();
    static boolean ready;

    public static void main(String[] args) throws InterruptedException {
        Thread starter = new Thread(() -> {
            synchronized (lock) {
                ready = true;
                lock.notifyAll();
            }
        });
        starter.start();
        synchronized (lock) {
            while (!ready) {
                lock.wait();
            }
        }
        starter.join();
        Point[] points = new Point[3_000_000];
        for (int i = 0; i < points.length; i++) {
            points[i] = new Point(i);
        }
        for (int round = 0; round < 3; round++) {
            for (Point p : points) {
                p.x = p.x * 3 + 1;
            }
        }
        long sum = 0;
        for (Point p : points) {
            sum += p.x;
        }
        synchronized (points[12345]) {
            sum += points[12345].x;
        }
        System.out.println(sum);
    }
}
