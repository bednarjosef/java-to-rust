// Dekker's test on the elements of the JDK's atomic arrays: in each round, each of two threads
// sets an element of its own and then reads the other thread's. The arrays' `get` and `set` are
// volatile accesses (`VarHandle.getVolatile` and `setVolatile`), which Java makes sequentially
// consistent, so in no round do both threads read the other's element before it is set. With
// acquire and release alone, a store may wait in the processor's store buffer while the load
// after it goes ahead, and both threads would read nothing in some rounds. The threads keep in
// step round by round through another atomic array, so that their stores and loads meet.
import java.util.concurrent.atomic.AtomicIntegerArray;
import java.util.concurrent.atomic.AtomicLongArray;
import java.util.concurrent.atomic.AtomicReferenceArray;

public class AtomicArrayDekker {
    static final int ROUNDS = 50_000;

    // The rounds in which neither thread saw the other's element set, with the elements of
    // kind `kind`: 0 for an `AtomicIntegerArray`, 1 for an `AtomicLongArray`, 2 for an
    // `AtomicReferenceArray`.
    static int bothMissed(int kind) throws InterruptedException {
        AtomicIntegerArray ints = new AtomicIntegerArray(2 * ROUNDS);
        AtomicLongArray longs = new AtomicLongArray(2 * ROUNDS);
        AtomicReferenceArray<String> names = new AtomicReferenceArray<>(2 * ROUNDS);
        AtomicIntegerArray step = new AtomicIntegerArray(2);
        boolean[][] missed = new boolean[2][ROUNDS];
        Thread[] threads = new Thread[2];
        for (int t = 0; t < 2; t++) {
            int me = t;
            threads[t] = new Thread(() -> {
                for (int r = 0; r < ROUNDS; r++) {
                    step.set(me, r);
                    while (step.get(1 - me) < r) {
                        Thread.onSpinWait();
                    }
                    int mine = 2 * r + me;
                    int theirs = 2 * r + 1 - me;
                    boolean seen;
                    if (kind == 0) {
                        ints.set(mine, 1);
                        seen = ints.get(theirs) != 0;
                    } else if (kind == 1) {
                        longs.set(mine, 1L);
                        seen = longs.get(theirs) != 0;
                    } else {
                        names.set(mine, "set");
                        seen = names.get(theirs) != null;
                    }
                    missed[me][r] = !seen;
                }
            });
        }
        for (Thread thread : threads) {
            thread.start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        int both = 0;
        for (int r = 0; r < ROUNDS; r++) {
            if (missed[0][r] && missed[1][r]) {
                both++;
            }
        }
        return both;
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println("AtomicIntegerArray: " + bothMissed(0) + " rounds of " + ROUNDS + " missed both");
        System.out.println("AtomicLongArray: " + bothMissed(1) + " rounds of " + ROUNDS + " missed both");
        System.out.println("AtomicReferenceArray: " + bothMissed(2) + " rounds of " + ROUNDS + " missed both");
    }
}
