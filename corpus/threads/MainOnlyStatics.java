// Statics that only the main thread touches next to statics that a worker touches: the worker
// initializes a class that main reads after `join` (its initializer runs once, and main sees what
// it set), and counts into another class's statics through a static method; main keeps counters
// of its own before and after starting the worker, and hands the worker an object that it fills.
public class MainOnlyStatics {
    static class MainCounters {
        static int started;
        static String label = makeLabel();
        static StringBuilder log = new StringBuilder();

        static String makeLabel() {
            started++;
            return "main-" + started;
        }
    }

    static class FirstUsedByWorker {
        static int initializations;
        static int value = compute();

        static int compute() {
            initializations++;
            return 6 * 7;
        }

        static int get() {
            return value;
        }
    }

    static class Hits {
        static int count;

        static void hit() {
            count++;
        }
    }

    static class Results {
        final int[] values = new int[3];
    }

    public static void main(String[] args) throws InterruptedException {
        MainCounters.log.append("before ").append(MainCounters.label);
        Results results = new Results();
        Thread worker = new Thread(() -> {
            for (int i = 0; i < 3; i++) {
                Hits.hit();
                results.values[i] = FirstUsedByWorker.get() + i;
            }
        });
        worker.start();
        MainCounters.started++;
        MainCounters.log.append(", while ").append(MainCounters.started);
        worker.join();
        MainCounters.log.append(", after ").append(Hits.count);
        System.out.println(MainCounters.log);
        System.out.println(results.values[0] + " " + results.values[1] + " " + results.values[2]);
        System.out.println(FirstUsedByWorker.value + " initialized " + FirstUsedByWorker.initializations + " time(s)");
        System.out.println(MainCounters.label + " " + MainCounters.started);
    }
}
