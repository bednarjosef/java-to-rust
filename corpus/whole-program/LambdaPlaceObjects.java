import java.lang.ref.WeakReference;
import java.util.HashSet;
import java.util.Set;
import java.util.function.Supplier;

// The one object of a lambda that captures nothing, which every evaluation of its place and every
// thread share: it can be locked, waited on, hashed by identity, kept in a set and held weakly,
// which never loses it, as the place keeps it.
public class LambdaPlaceObjects {
    interface Op {
        int apply(int x);
    }

    static Op twice() {
        return x -> x * 2;
    }

    static Supplier<String> name() {
        return () -> "name";
    }

    public static void main(String[] args) throws Exception {
        Op locked = twice();
        synchronized (locked) {
            System.out.println("locked " + locked.apply(5));
        }
        System.out.println(System.identityHashCode(twice()) == System.identityHashCode(locked));
        System.out.println(locked.hashCode() == System.identityHashCode(locked));
        Op other = x -> x * 2;
        System.out.println(locked.equals(twice()) + " " + locked.equals(other) + " " + (locked.getClass() == twice().getClass()));
        Set<Object> seen = new HashSet<>();
        seen.add(locked);
        seen.add(twice());
        seen.add(name());
        seen.add(name());
        seen.add(other);
        System.out.println(seen.size() + " " + seen.contains(twice()) + " " + seen.contains(name()));
        WeakReference<Op> held = new WeakReference<>(twice());
        System.gc();
        System.out.println(held.get() == twice());
        Op[] fromThread = new Op[1];
        Thread worker = new Thread(() -> {
            Op mine = twice();
            synchronized (mine) {
                fromThread[0] = mine;
                mine.notifyAll();
            }
        });
        synchronized (locked) {
            worker.start();
            while (fromThread[0] == null) {
                locked.wait(10);
            }
        }
        worker.join();
        System.out.println((fromThread[0] == locked) + " " + fromThread[0].apply(21) + " " + name().get());
    }
}
