import java.util.concurrent.locks.LockSupport;

// `LockSupport.park` asks `Thread.currentThread().isVirtual()`, which is false in a translation
// (no virtual threads), so it parks the platform thread. A permit given first lets it return.
public class ParkWithPermit {
    public static void main(String[] args) throws InterruptedException {
        LockSupport.unpark(Thread.currentThread());
        LockSupport.park();
        System.out.println("returned with the permit");
        LockSupport.parkNanos(1000);
        System.out.println("returned after the wait");
        Thread main = Thread.currentThread();
        Thread waker = new Thread(() -> LockSupport.unpark(main));
        waker.start();
        LockSupport.park(main);
        waker.join();
        System.out.println("woken by another thread");
    }
}
