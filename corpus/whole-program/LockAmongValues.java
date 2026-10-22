// A plain `new Object()` as a lock, waited on and notified across threads, in a program whose
// `Object`s also hold strings and boxes, which have no monitor of their own in the translation.
import java.util.*;
public class LockAmongValues {
    static final Object LOCK = new Object();
    static int count;
    public static void main(String[] args) throws Exception {
        List<Object> things = new ArrayList<>(List.of("a", 1, 2.5));
        things.add(LOCK);
        Thread t = new Thread(() -> {
            synchronized (LOCK) {
                count++;
                LOCK.notifyAll();
            }
        });
        synchronized (LOCK) {
            t.start();
            while (count == 0) {
                LOCK.wait();
            }
        }
        t.join();
        System.out.println(count + " " + things.size() + " " + (things.get(3) == LOCK));
    }
}
