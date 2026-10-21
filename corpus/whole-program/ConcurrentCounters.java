// Four threads count into the JDK's ConcurrentHashMap at once. Its bins are claimed with an
// atomic compare-and-set of an element of the table (Unsafe at `((long) i << ASHIFT) + ABASE`),
// its count is kept with compare-and-set at field offsets, and a bin that holds nodes is locked
// with `synchronized` on its first node. No update is lost, so the totals are the same on every
// run.
import java.util.concurrent.ConcurrentHashMap;

public class ConcurrentCounters {
    public static void main(String[] args) throws InterruptedException {
        ConcurrentHashMap<Integer, Integer> counts = new ConcurrentHashMap<>();
        ConcurrentHashMap<String, Long> sums = new ConcurrentHashMap<>();
        counts.put(-1, 0);
        Thread[] workers = new Thread[4];
        for (int t = 0; t < workers.length; t++) {
            int id = t;
            workers[t] = new Thread(() -> {
                for (int i = 0; i < 3000; i++) {
                    counts.merge(i % 150, 1, Integer::sum);
                    sums.merge(i % 2 == 0 ? "even" : "odd", (long) i, Long::sum);
                    counts.computeIfPresent(-1, (k, v) -> v + 1);
                }
                counts.putIfAbsent(1000 + id, id * 10);
            });
        }
        for (Thread w : workers) {
            w.start();
        }
        for (Thread w : workers) {
            w.join();
        }
        int total = 0;
        for (int k = 0; k < 150; k++) {
            total += counts.get(k);
        }
        System.out.println(counts.size() + " " + total + " " + counts.get(7) + " " + counts.get(-1));
        System.out.println(sums.get("even") + " " + sums.get("odd"));
        System.out.println(counts.get(1000) + " " + counts.get(1003));
    }
}
