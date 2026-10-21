import java.util.function.Function;
import java.util.function.UnaryOperator;

// A UnaryOperator that a thread runs, beside Functions that only the main thread calls: the two
// interfaces share their closures, so both are Arcs.
public class OperatorsAcrossThreads {
    static String describe(Function<String, String> f, String s) {
        return "[" + f.apply(s) + "]";
    }

    public static void main(String[] args) throws InterruptedException {
        String suffix = "!";
        UnaryOperator<String> shout = s -> s.toUpperCase() + suffix;
        String[] out = new String[2];
        Thread worker = new Thread(() -> {
            out[0] = shout.apply("hello");
            out[1] = shout.apply("threads");
        });
        worker.start();
        worker.join();
        Function<String, String> quiet = s -> s.toLowerCase();
        System.out.println(out[0] + " " + out[1]);
        System.out.println(describe(quiet, "MAIN") + " " + describe(shout, "main"));
    }
}
