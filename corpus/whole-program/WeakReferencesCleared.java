// References whose referents nothing else keeps: the collector clears them and puts those with a
// queue on it, a `WeakHashMap` loses the entries of such keys, a soft referent read lately stays,
// and a referent that something keeps is never cleared. `System.gc()` asks Java's collector to
// run; the translation's collector runs as the program allocates, so each step allocates garbage
// until what it waits for happens, a bounded number of times, and prints only whether it did.
import java.lang.ref.PhantomReference;
import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.SoftReference;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.WeakHashMap;

public class WeakReferencesCleared {
    static class Payload {
        final int value;
        final long[] data = new long[8];

        Payload(int value) {
            this.value = value;
        }
    }

    static Object sink;

    /** Allocates about 16 MB of garbage, and asks Java's collector to run. */
    static void churn() {
        for (int i = 0; i < 4096; i++) {
            sink = new byte[4096];
        }
        sink = null;
        System.gc();
    }

    static List<WeakReference<Payload>> weakReferences(int n, ReferenceQueue<Payload> queue) {
        List<WeakReference<Payload>> references = new ArrayList<>();
        for (int i = 0; i < n; i++) {
            references.add(new WeakReference<>(new Payload(i), queue));
        }
        return references;
    }

    static int cleared(List<? extends Reference<Payload>> references) {
        int n = 0;
        for (Reference<Payload> r : references) {
            if (r.refersTo(null)) {
                n++;
            }
        }
        return n;
    }

    /** A weak reference that never leaves the method, to an object that it holds strongly too. */
    static int local(int value) {
        Payload payload = new Payload(value);
        WeakReference<Payload> reference = new WeakReference<>(payload);
        churn();
        Payload read = reference.get();
        return read == null ? -1 : read.value + payload.value;
    }

    public static void main(String[] args) {
        ReferenceQueue<Payload> queue = new ReferenceQueue<>();
        List<WeakReference<Payload>> weak = weakReferences(100, queue);
        List<Payload> held = new ArrayList<>();
        List<WeakReference<Payload>> kept = new ArrayList<>();
        for (int i = 0; i < 100; i++) {
            Payload p = new Payload(i);
            held.add(p);
            kept.add(new WeakReference<>(p));
        }
        for (int round = 0; round < 200 && cleared(weak) < 90; round++) {
            churn();
        }
        System.out.println("weak referents cleared: " + (cleared(weak) >= 90));
        System.out.println("held referents cleared: " + cleared(kept));
        int sum = 0;
        for (WeakReference<Payload> r : kept) {
            sum += r.get().value;
        }
        System.out.println("held referents: " + sum + " " + held.size());

        // The cleared references come out of their queue, each once.
        int polled = 0;
        boolean allCleared = true;
        for (int round = 0; round < 200 && polled < 90; round++) {
            Reference<? extends Payload> r;
            while ((r = queue.poll()) != null) {
                polled++;
                allCleared &= r.refersTo(null) && weak.contains(r);
            }
            if (polled < 90) {
                churn();
            }
        }
        System.out.println("queued: " + (polled >= 90) + " " + allCleared + " " + (polled <= 100));

        // A phantom reference's referent is never read, and it is queued once the referent dies.
        ReferenceQueue<Payload> phantoms = new ReferenceQueue<>();
        List<PhantomReference<Payload>> phantom = new ArrayList<>();
        for (int i = 0; i < 50; i++) {
            phantom.add(new PhantomReference<>(new Payload(i), phantoms));
        }
        int phantomPolled = 0;
        for (int round = 0; round < 200 && phantomPolled < 45; round++) {
            churn();
            while (phantoms.poll() != null) {
                phantomPolled++;
            }
        }
        System.out.println("phantoms: " + (phantom.get(0).get() == null) + " " + (phantomPolled >= 45));

        // A `WeakHashMap` loses the entries whose keys nothing else keeps.
        Map<Payload, String> map = new WeakHashMap<>();
        List<Payload> keys = new ArrayList<>();
        for (int i = 0; i < 1000; i++) {
            Payload key = new Payload(i);
            if (i % 10 == 0) {
                keys.add(key);
            }
            map.put(key, "v" + i);
        }
        for (int round = 0; round < 200 && map.size() > 150; round++) {
            churn();
        }
        System.out.println("map: " + (map.size() <= 150) + " " + (map.size() >= 100));
        int found = 0;
        for (Payload key : keys) {
            if (("v" + key.value).equals(map.get(key))) {
                found++;
            }
        }
        System.out.println("kept keys found: " + found);

        // A soft referent read lately is kept while memory is plentiful.
        SoftReference<Payload> soft = new SoftReference<>(new Payload(7));
        boolean softKept = true;
        for (int round = 0; round < 5; round++) {
            churn();
            softKept &= soft.get() != null;
        }
        System.out.println("soft kept: " + softKept);

        System.out.println("local: " + local(21));
    }
}
