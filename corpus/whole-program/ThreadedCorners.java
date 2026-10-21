import java.util.concurrent.atomic.AtomicReference;

/** Corners that ForkJoinPool's code reaches, in a program with threads. */
public class ThreadedCorners {
    static final Object[] EMPTY = {};

    static class Failure extends RuntimeException {
        Failure(String message) { super(message); }
    }

    /** No object of it is created: a value of it is always null. */
    abstract static class Link {
        Link next;
    }

    static class Chain extends Link {
        int weight;
    }

    static class Box {
        final String name;
        Box(String name) { this.name = name; }
    }

    static final AtomicReference<Box> SLOT = new AtomicReference<>(new Box("first"));

    /** Throws `e`, which may be null: then a NullPointerException. */
    static void raise(RuntimeException e) {
        throw e;
    }

    static int length(Link link) {
        int n = 0;
        while (link != null) {
            link = link.next;
            n++;
        }
        return n;
    }

    /** Replaces the slot's box when it still holds `expected`, giving what it held. */
    static Box exchange(Box expected, Box replacement) {
        Box[] seen = new Box[1];
        SLOT.updateAndGet(b -> { seen[0] = b; return b == expected ? replacement : b; });
        return seen[0];
    }

    public static void main(String[] args) throws Exception {
        int[] sizes = new int[1];
        Thread t = new Thread(() -> sizes[0] = EMPTY.length);
        t.start();
        t.join();
        System.out.println("empty: " + sizes[0]);

        for (RuntimeException e : new RuntimeException[] { new Failure("thrown"), null }) {
            try {
                raise(e);
            } catch (Failure f) {
                System.out.println("caught failure " + f.getMessage());
            } catch (NullPointerException n) {
                System.out.println("caught a null pointer");
            }
        }

        System.out.println("links: " + length(null));

        Box a = SLOT.get();
        System.out.println("same: " + (a == (a = exchange(a, new Box("second")))) + " " + SLOT.get().name);
        Box stale = new Box("stale");
        System.out.println("same: " + (stale == (stale = exchange(stale, new Box("third")))) + " " + SLOT.get().name);

        int[] numbers = {1, 2};
        Object asObject = numbers;
        System.out.println("arrays: " + asObject.equals(numbers) + " " + asObject.equals(new int[] {1, 2}));
    }
}
