import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

// Static finals read while their own class is being initialized, in a program whose threads use
// the classes: a read before the assignment sees null or 0, as Java's does, whether the
// initializer reads the static itself, calls a method that reads it, or starts the
// initialization of another class that reads it back (a cycle of two classes). After the
// initialization every thread sees the values set.
public class StaticFinalReadDuringInitializer {
    static final class Early {
        static final String BEFORE = describe();
        static final int[] NUMBERS;
        static final String LABEL;
        static final String AFTER;

        static {
            // Java reads a blank final before its assignment by its qualified name only.
            String seen = "numbers " + (Early.NUMBERS == null ? "unset" : "set") + ", label " + Early.LABEL;
            NUMBERS = new int[] {4, 5, 6};
            LABEL = "early";
            AFTER = seen + " / " + describe();
        }

        static String describe() {
            return "label=" + LABEL + " numbers=" + (NUMBERS == null ? "null" : String.valueOf(NUMBERS.length));
        }
    }

    static final class First {
        static final String NAME;
        static final String SEEN_FROM_SECOND;

        static {
            SEEN_FROM_SECOND = Second.FIRST_NAME_SEEN;
            NAME = "first";
        }
    }

    static final class Second {
        static final String FIRST_NAME_SEEN;
        static final Second ITSELF;
        static final String TAG = "second";

        static {
            // `First` is being initialized by this thread when it gets here from `First`.
            FIRST_NAME_SEEN = "second saw " + First.NAME;
            ITSELF = new Second();
        }

        String tag() {
            return TAG + (ITSELF == this ? " (itself)" : " (another)");
        }
    }

    public static void main(String[] args) throws InterruptedException {
        // `main` starts the cycle at `First`.
        System.out.println(First.NAME + ", " + First.SEEN_FROM_SECOND);
        List<String> seen = Collections.synchronizedList(new ArrayList<>());
        List<Thread> threads = new ArrayList<>();
        for (int i = 0; i < 4; i++) {
            threads.add(new Thread(() -> {
                seen.add(Early.BEFORE + " | " + Early.AFTER + " | " + Early.describe() + " | " + Early.NUMBERS[2]);
                seen.add(Second.ITSELF.tag() + ", " + Second.FIRST_NAME_SEEN);
            }));
        }
        for (Thread t : threads) {
            t.start();
        }
        for (Thread t : threads) {
            t.join();
        }
        Collections.sort(seen);
        for (String s : seen) {
            System.out.println(s);
        }
    }
}
