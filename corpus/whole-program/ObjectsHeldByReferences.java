import java.lang.ref.Cleaner;
import java.lang.ref.PhantomReference;
import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.WeakReference;
import java.util.WeakHashMap;

/**
 * Objects that a reference holds in its own fields, made in a method that returns before they
 * are read: a cleaner's action, the value of a `WeakHashMap` entry, and a field of subclasses of
 * `PhantomReference` and `WeakReference`. Each lives as long as the reference, though the frame
 * that made it is gone and others have taken its place on the stack.
 */
public class ObjectsHeldByReferences {
    static final class Note {
        final String text;
        final int number;

        Note(String text, int number) {
            this.text = text;
            this.number = number;
        }
    }

    static final class Action implements Runnable {
        private final Note note;
        private final int[] count;

        Action(Note note, int[] count) {
            this.note = note;
            this.count = count;
        }

        @Override
        public void run() {
            count[0]++;
            System.out.println("cleaned " + note.text + " " + note.number + ", " + count[0] + " so far");
        }
    }

    static final class Resource {
        private final Cleaner.Cleanable cleanable;

        Resource(Cleaner cleaner, String name, int number, int[] count) {
            cleanable = cleaner.register(this, new Action(new Note(name, number), count));
        }

        void close() {
            cleanable.clean();
        }
    }

    static final class Tracked extends PhantomReference<Object> {
        final Note note;

        Tracked(Object referent, ReferenceQueue<Object> queue, Note note) {
            super(referent, queue);
            this.note = note;
        }
    }

    static final class Labeled extends WeakReference<Object> {
        final Note note;

        Labeled(Object referent, Note note) {
            super(referent);
            this.note = note;
        }
    }

    static Tracked track(Object referent, ReferenceQueue<Object> queue, String text) {
        return new Tracked(referent, queue, new Note(text, text.length()));
    }

    static Labeled label(Object referent, String text) {
        return new Labeled(referent, new Note(text, text.length()));
    }

    static void fill(WeakHashMap<Object, Note> map, Object key, String text) {
        map.put(key, new Note(text, text.length()));
    }

    static Runnable printer(Cleaner cleaner, Object owner, String text) {
        Runnable action = () -> System.out.println("printed " + text);
        cleaner.register(owner, action);
        return action;
    }

    /** Fills the stack where the frames above were with other values. */
    static long churn(int depth) {
        if (depth == 0) {
            return 1;
        }
        long a = depth * 31L;
        long b = a ^ 0x5555_5555_5555L;
        long c = b + depth;
        return churn(depth - 1) + (a ^ b ^ c) % 7;
    }

    public static void main(String[] args) {
        Cleaner cleaner = Cleaner.create();
        int[] count = {0};
        Resource first = new Resource(cleaner, "first", 1, count);
        Resource second = new Resource(cleaner, "second", 2, count);
        System.out.println("churned " + churn(300));
        second.close();
        first.close();
        // A second close does nothing.
        first.close();

        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object kept = new Object();
        Tracked tracked = track(kept, queue, "tracked");
        Labeled labeled = label(kept, "labeled");
        System.out.println("churned " + churn(300));
        System.out.println(tracked.note.text + " " + tracked.note.number);
        System.out.println(labeled.note.text + " " + labeled.note.number + " " + (labeled.get() == kept));

        WeakHashMap<Object, Note> map = new WeakHashMap<>();
        Object key = new Object();
        fill(map, key, "weak value");
        System.out.println("churned " + churn(300));
        Note value = map.get(key);
        System.out.println(value.text + " " + value.number + " of " + map.size());

        Object owner = new Object();
        Runnable action = printer(cleaner, owner, "from a lambda");
        System.out.println("churned " + churn(300));
        action.run();
        Reference.reachabilityFence(kept);
        Reference.reachabilityFence(key);
        Reference.reachabilityFence(owner);
    }
}
