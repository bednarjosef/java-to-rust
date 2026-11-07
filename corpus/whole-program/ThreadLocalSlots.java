import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

// Thread-locals found by their slot numbers (the translator's patch of the JDK's ThreadLocal) next
// to those found by hashing: a `withInitial` value made once per thread, a subclass's
// `initialValue`, `set` (of null too), `get` and `remove`, values seen by their own thread alone,
// an inherited value, thread-locals made by several threads at once, and more thread-locals than
// have slot numbers in one thread's map, some removed and some left for the collector.
public class ThreadLocalSlots {
    static final AtomicInteger made = new AtomicInteger();
    static final ThreadLocal<int[]> counter = ThreadLocal.withInitial(() -> {
        made.incrementAndGet();
        return new int[1];
    });

    static final class Named extends ThreadLocal<String> {
        final AtomicInteger initials = new AtomicInteger();

        @Override
        protected String initialValue() {
            initials.incrementAndGet();
            return Thread.currentThread().getName();
        }
    }

    static final Named named = new Named();
    static final InheritableThreadLocal<String> inherited = new InheritableThreadLocal<>() {
        @Override
        protected String childValue(String parent) {
            return parent + "+";
        }
    };

    public static void main(String[] args) throws Exception {
        counter.get()[0] += 5;
        counter.get()[0] += 1;
        System.out.println("main counter " + counter.get()[0] + ", made " + made.get());
        System.out.println("main named " + named.get() + " " + named.get() + ", initials " + named.initials.get());
        named.set(null);
        System.out.println("after set(null): " + named.get() + ", initials " + named.initials.get());
        named.remove();
        System.out.println("after remove: " + named.get() + ", initials " + named.initials.get());
        named.set("main's");
        inherited.set("main");

        // More thread-locals than get slot numbers, all in the main thread's map.
        List<ThreadLocal<Integer>> many = new ArrayList<>();
        for (int i = 0; i < 5000; i++) {
            ThreadLocal<Integer> local = new ThreadLocal<>();
            local.set(i);
            many.add(local);
        }
        for (int i = 0; i < many.size(); i += 3) {
            many.get(i).remove();
        }
        System.out.println("many: " + summary(many));
        for (int i = 1; i < many.size(); i += 3) {
            many.get(i).set(-i);
        }
        System.out.println("many after sets: " + summary(many));

        String[] seen = new String[4];
        Thread[] workers = new Thread[seen.length];
        for (int w = 0; w < workers.length; w++) {
            int id = w;
            workers[w] = new Thread(() -> {
                for (int i = 0; i < 1000; i++) {
                    counter.get()[0] += id;
                }
                String first = named.get();
                named.set("worker " + id);
                // The main thread's values are not this thread's.
                int fromMain = 0;
                for (ThreadLocal<Integer> local : many) {
                    if (local.get() != null) {
                        fromMain++;
                    }
                }
                many.get(id).set(id * 100);
                // Thread-locals made by all the workers at once, each with its own number.
                List<ThreadLocal<String>> own = new ArrayList<>();
                for (int i = 0; i < 300; i++) {
                    ThreadLocal<String> local = ThreadLocal.withInitial(() -> "initial");
                    if (i % 2 == 0) {
                        local.set(id + ":" + i);
                    }
                    own.add(local);
                }
                int matching = 0;
                for (int i = 0; i < own.size(); i++) {
                    String want = i % 2 == 0 ? id + ":" + i : "initial";
                    if (own.get(i).get().equals(want)) {
                        matching++;
                    }
                }
                seen[id] = first + " / " + named.get() + " / counter " + counter.get()[0] + " / from main "
                        + fromMain + " / own " + many.get(id).get() + " / made here " + matching + " / inherited "
                        + inherited.get();
            }, "w" + w);
            workers[w].start();
        }
        for (Thread worker : workers) {
            worker.join();
        }
        for (String s : seen) {
            System.out.println(s);
        }
        System.out.println("main after workers: " + named.get() + " " + counter.get()[0] + " " + many.get(2).get()
                + " " + inherited.get() + ", made " + made.get() + ", initials " + named.initials.get());

        // Drop most of the thread-locals for the collector, then fill the map with new ones: the
        // map finds the dropped ones stale (if the collector cleared them) and expunges them while
        // the live ones keep their values.
        List<ThreadLocal<Integer>> kept = new ArrayList<>();
        for (int i = 0; i < many.size(); i += 10) {
            kept.add(many.get(i));
        }
        many.clear();
        System.gc();
        List<ThreadLocal<Integer>> fresh = new ArrayList<>();
        for (int i = 0; i < 3000; i++) {
            ThreadLocal<Integer> local = new ThreadLocal<>();
            local.set(i * 2);
            fresh.add(local);
        }
        System.out.println("kept: " + summary(kept) + ", fresh: " + summary(fresh));
        for (ThreadLocal<Integer> local : fresh) {
            local.remove();
        }
        System.out.println("fresh removed: " + summary(fresh) + ", kept: " + summary(kept));
        counter.remove();
        System.out.println("counter again " + counter.get()[0] + ", made " + made.get());
    }

    /** How many of the thread-locals hold a value in this thread, and the sum of the values. */
    static String summary(List<ThreadLocal<Integer>> locals) {
        int present = 0;
        long sum = 0;
        for (ThreadLocal<Integer> local : locals) {
            Integer value = local.get();
            if (value != null) {
                present++;
                sum += value;
            }
        }
        return present + " present, sum " + sum;
    }
}
