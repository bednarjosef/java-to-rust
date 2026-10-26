import java.util.concurrent.atomic.AtomicIntegerFieldUpdater;
import java.util.concurrent.atomic.AtomicReferenceFieldUpdater;

// Field updaters of generic classes, cast to raw updaters or made in a static block's local, as
// netty's `ResourceLeakDetector.DefaultResourceLeak` and `Recycler.DefaultHandle` make theirs.
public class UpdaterCastsAndLocals {
    static final class Record {
        final String hint;
        final Record next;

        Record(String hint, Record next) {
            this.hint = hint;
            this.next = next;
        }
    }

    static final class Leak<T> {
        @SuppressWarnings({"unchecked", "rawtypes"})
        private static final AtomicReferenceFieldUpdater<Leak<?>, Record> headUpdater =
                (AtomicReferenceFieldUpdater) AtomicReferenceFieldUpdater.newUpdater(Leak.class, Record.class, "head");

        @SuppressWarnings({"unchecked", "rawtypes"})
        private static final AtomicIntegerFieldUpdater<Leak<?>> droppedUpdater =
                (AtomicIntegerFieldUpdater) AtomicIntegerFieldUpdater.newUpdater(Leak.class, "dropped");

        private volatile Record head;
        private volatile int dropped;

        void record(String hint) {
            Record old;
            do {
                old = headUpdater.get(this);
            } while (!headUpdater.compareAndSet(this, old, new Record(hint, old)));
            if (hint.isEmpty()) {
                droppedUpdater.incrementAndGet(this);
            }
        }

        String records() {
            StringBuilder b = new StringBuilder();
            for (Record r = head; r != null; r = r.next) {
                b.append(r.hint).append(' ');
            }
            return b.append(dropped).toString();
        }
    }

    static final class Handle<T> {
        private static final AtomicIntegerFieldUpdater<Handle<?>> STATE_UPDATER;

        static {
            AtomicIntegerFieldUpdater<?> updater = AtomicIntegerFieldUpdater.newUpdater(Handle.class, "state");
            //noinspection unchecked
            STATE_UPDATER = (AtomicIntegerFieldUpdater<Handle<?>>) updater;
        }

        private volatile int state;

        boolean claim() {
            return STATE_UPDATER.compareAndSet(this, 0, 1);
        }

        void release() {
            STATE_UPDATER.lazySet(this, 0);
        }
    }

    public static void main(String[] args) {
        Leak<String> leak = new Leak<>();
        leak.record("a");
        leak.record("");
        leak.record("b");
        System.out.println(leak.records());
        Handle<Object> handle = new Handle<>();
        System.out.println(handle.claim() + " " + handle.claim());
        handle.release();
        System.out.println(handle.claim() + " " + handle.state);
    }
}
