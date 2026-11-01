import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.IntUnaryOperator;

// Catches of Throwable and Exception far from where their exceptions are thrown, in a program
// that the split test cuts into crates: the exceptions unwind through code of the body crates,
// the JDK's collections and lambdas, to the catch in the binary's crate.
public class CatchAllsAcrossCrates {
    interface Task {
        int run(int n) throws Exception;
    }

    static class Failure extends RuntimeException {
        Failure(String message) {
            super(message);
        }
    }

    static final Map<String, Integer> counts = new HashMap<>();

    static int step(int n) {
        if (n % 5 == 4) {
            throw new Failure("step " + n);
        }
        if (n % 7 == 6) {
            List<Integer> small = new ArrayList<>();
            return small.get(n);
        }
        return n * 3;
    }

    static int through(List<IntUnaryOperator> ops, int n) {
        int value = n;
        for (IntUnaryOperator op : ops) {
            value = op.applyAsInt(value);
        }
        return value;
    }

    static void count(String key) {
        counts.merge(key, 1, Integer::sum);
    }

    public static void main(String[] args) {
        List<IntUnaryOperator> ops = new ArrayList<>();
        ops.add(x -> x + 1);
        ops.add(CatchAllsAcrossCrates::step);
        ops.add(x -> 100 / (x % 11));
        List<Task> tasks = new ArrayList<>();
        tasks.add(n -> through(ops, n));
        tasks.add(n -> {
            if (n == 3) {
                throw new Exception("checked " + n);
            }
            return n;
        });
        int sum = 0;
        for (int i = 0; i < 40; i++) {
            for (Task t : tasks) {
                try {
                    sum += t.run(i);
                    count("ok");
                } catch (Throwable e) {
                    count(e.getClass().getSimpleName());
                    if (i < 12) {
                        System.out.println(i + ": " + e);
                    }
                }
            }
        }
        System.out.println("sum " + sum);
        System.out.println(new java.util.TreeMap<>(counts));
        try {
            Object o = counts.get("missing").toString();
            System.out.println(o);
        } catch (Exception e) {
            System.out.println("null " + (e instanceof NullPointerException));
        }
    }
}
