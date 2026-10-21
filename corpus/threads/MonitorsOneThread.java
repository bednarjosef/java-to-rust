// Monitors in a program with one thread: holdsLock tells whether the thread holds an object's
// monitor, a timed wait returns when its time is up, and wait or notify on an object whose
// monitor the thread does not hold throws IllegalMonitorStateException.
public class MonitorsOneThread {
    static class Box {
        int value;

        synchronized void set(int v) {
            value = v;
            System.out.println("inside set: " + Thread.holdsLock(this));
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Box box = new Box();
        System.out.println("before: " + Thread.holdsLock(box));
        box.set(4);
        synchronized (box) {
            System.out.println("in block: " + Thread.holdsLock(box));
            synchronized (box) {
                box.set(5);
            }
            System.out.println("still: " + Thread.holdsLock(box));
            long start = System.nanoTime();
            box.wait(20);
            System.out.println("waited: " + ((System.nanoTime() - start) >= 20_000_000L));
            box.notifyAll();
        }
        System.out.println("after: " + Thread.holdsLock(box) + " " + box.value);
        try {
            box.notify();
        } catch (IllegalMonitorStateException e) {
            System.out.println("notify: " + e.getMessage());
        }
        try {
            box.wait();
        } catch (IllegalMonitorStateException e) {
            System.out.println("wait: " + e.getMessage());
        }
        try {
            synchronized (box) {
                box.wait(-1);
            }
        } catch (IllegalArgumentException e) {
            System.out.println("negative: " + e.getMessage());
        }
    }
}
