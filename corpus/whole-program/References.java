// Soft, weak and phantom references, translated from the JDK's java.lang.ref, with the referent
// natives as code on the referent. A translated program has no collector that clears a
// reference behind its back, as on a JVM whose collector never needs the memory: the referents
// here stay strongly reachable, so Java gives the same output. Only `clear` changes a
// reference, and a queue stays empty. (`enqueue` waits for `Object.notifyAll`.)
import java.lang.ref.PhantomReference;
import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.SoftReference;
import java.lang.ref.WeakReference;

public class References {
    // A reference with a value of its own, as `ThreadLocal`'s entries and the JDK's caches have.
    static class Key {
        final String name;

        Key(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    static class Entry extends WeakReference<Key> {
        final int count;

        Entry(Key key, int count, ReferenceQueue<Key> queue) {
            super(key, queue);
            this.count = count;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        ReferenceQueue<Key> queue = new ReferenceQueue<>();
        Key text = new Key("kept");
        SoftReference<Key> soft = new SoftReference<>(text);
        Entry entry = new Entry(text, 3, queue);
        System.out.println(soft.get() + " " + entry.get() + " " + entry.count);
        System.out.println(soft.refersTo(text) + " " + entry.refersTo(null));

        Reference<? extends Key> taken = queue.poll();
        System.out.println((taken == null) + " " + entry.isEnqueued());
        entry.clear();
        System.out.println(entry.get() == null);

        soft.clear();
        System.out.println(soft.get() == null);
        System.out.println(soft.refersTo(null) + " " + soft.refersTo(text));

        PhantomReference<Key> phantom = new PhantomReference<>(text, queue);
        System.out.println((phantom.get() == null) + " " + phantom.refersTo(text));
        phantom.clear();
        System.out.println(phantom.refersTo(text));
        System.out.println(text);
    }
}
