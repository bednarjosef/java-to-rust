// The JDK's AtomicInteger and AtomicLong, classes that extend Number and update their value
// through Unsafe's atomic forms at a field offset (getAndAddInt, compareAndSetInt, getAndSetInt):
// each update is a method of the class, which runs with the object's lock held, so four threads
// incrementing one counter lose no update.
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

public class AtomicNumbers {
    static final AtomicInteger shared = new AtomicInteger();

    static class Worker extends Thread {
        @Override
        public void run() {
            for (int i = 0; i < 10000; i++) {
                shared.incrementAndGet();
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        AtomicInteger counter = new AtomicInteger(5);
        counter.incrementAndGet();
        counter.addAndGet(10);
        System.out.println(counter.compareAndSet(16, 20) + " " + counter.compareAndSet(16, 30) + " " + counter);
        System.out.println(counter.getAndSet(-1) + " " + counter.getAndIncrement() + " " + counter.get());
        AtomicLong big = new AtomicLong(Long.MAX_VALUE);
        System.out.println(big.incrementAndGet() + " " + big.getAndAdd(3) + " " + big);
        Number n = counter;
        Number m = big;
        System.out.println(n.intValue() + " " + n.doubleValue() + " " + m.byteValue() + " " + m.shortValue());
        Worker[] workers = new Worker[4];
        for (int i = 0; i < workers.length; i++) {
            workers[i] = new Worker();
            workers[i].start();
        }
        for (Worker w : workers) {
            w.join();
        }
        System.out.println(shared.get());
    }
}
