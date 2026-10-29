import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

// A class whose static initialization fails after it has set some of its static finals, used
// first by several threads at once: the thread that runs it gets `ExceptionInInitializerError`,
// every other use `NoClassDefFoundError`, and no thread reads the statics it set. A class that
// initializes fine next to it keeps its values, read with no lock by every thread.
public class StaticFinalFailedInitializer {
    static final class Broken {
        static final int[] TABLE = {1, 2, 3};
        static final String NAME = "broken";
        static final Object VALUE = fail();

        static Object fail() {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                throw new IllegalStateException(e);
            }
            if (TABLE.length == 3) {
                throw new IllegalArgumentException("no value for " + NAME);
            }
            return TABLE;
        }
    }

    static final class Fine {
        static final int[] TABLE;
        static final String NAME;

        static {
            int[] table = new int[16];
            for (int i = 0; i < table.length; i++) {
                table[i] = i * i;
            }
            TABLE = table;
            NAME = "fine";
        }
    }

    public static void main(String[] args) throws InterruptedException {
        List<String> seen = Collections.synchronizedList(new ArrayList<>());
        List<Thread> threads = new ArrayList<>();
        for (int i = 0; i < 4; i++) {
            int id = i;
            threads.add(new Thread(() -> {
                try {
                    seen.add("value " + Broken.VALUE);
                } catch (ExceptionInInitializerError e) {
                    seen.add("ExceptionInInitializerError: " + e.getCause().getMessage());
                } catch (NoClassDefFoundError e) {
                    seen.add("NoClassDefFoundError");
                }
                int sum = 0;
                for (int j = 0; j < 1000; j++) {
                    sum += Fine.TABLE[(j + id) & 15];
                }
                seen.add(Fine.NAME + " " + sum);
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
        try {
            System.out.println(Broken.TABLE.length);
        } catch (NoClassDefFoundError e) {
            System.out.println("later: " + e.getMessage());
        }
    }
}
