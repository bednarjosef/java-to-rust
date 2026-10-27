import java.util.concurrent.TimeUnit;

// Joins with a timeout in milliseconds and nanoseconds, as `TimeUnit.timedJoin` makes them: a
// thread that ends in time, one that does not, and the arguments `join` refuses.
public class TimedJoins {
    public static void main(String[] args) throws Exception {
        Thread quick = new Thread(() -> System.out.println("quick ran"));
        quick.start();
        TimeUnit.SECONDS.timedJoin(quick, 10);
        System.out.println("quick alive " + quick.isAlive());

        Object gate = new Object();
        Thread slow = new Thread(() -> {
            synchronized (gate) {
                try {
                    gate.wait();
                } catch (InterruptedException e) {
                    System.out.println("slow interrupted");
                }
            }
        });
        slow.start();
        TimeUnit.MICROSECONDS.timedJoin(slow, 1500);
        slow.join(0, 1);
        System.out.println("slow alive " + slow.isAlive());
        slow.interrupt();
        slow.join(5000, 999999);
        System.out.println("slow alive " + slow.isAlive());

        TimeUnit.SECONDS.timedJoin(slow, 0);
        try {
            slow.join(-1, 0);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        try {
            slow.join(1, 1000000);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
    }
}
