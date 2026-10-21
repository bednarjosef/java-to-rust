// The JDK's ThreadLocal, whose values each thread keeps in a map of its own (the translator's
// Thread has the field for it). A new thread starts from the initial value, a thread's `set`
// is seen by that thread alone, and `remove` brings back the initial value.
public class ThreadLocals {
    static final ThreadLocal<Integer> slot = ThreadLocal.withInitial(() -> 7);
    static final ThreadLocal<Tally> tally = new ThreadLocal<>();

    static final class Tally {
        int count;
    }

    public static void main(String[] args) throws InterruptedException {
        slot.set(1);
        int[] seen = new int[3];
        Thread[] workers = new Thread[3];
        for (int t = 0; t < workers.length; t++) {
            int id = t;
            workers[t] = new Thread(() -> {
                int first = slot.get();
                slot.set(10 * (id + 1));
                for (int i = 0; i < 1000; i++) {
                    slot.set(slot.get() + 1);
                }
                seen[id] = first + slot.get();
                Tally mine = new Tally();
                tally.set(mine);
                tally.get().count += id;
            });
            workers[t].start();
        }
        for (Thread w : workers) {
            w.join();
        }
        System.out.println(slot.get() + " " + seen[0] + " " + seen[1] + " " + seen[2] + " " + tally.get());
        slot.remove();
        System.out.println(slot.get());
    }
}
