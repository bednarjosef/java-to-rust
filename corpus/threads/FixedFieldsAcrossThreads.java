import java.util.function.IntUnaryOperator;

// Objects that threads share, whose final fields hold values that are more than a word: a class,
// a lambda, a string. The fields are read where they are, without a lock, once the constructor
// has written them; the constructor itself reads them back after writing them, and so does a
// method it calls.
public class FixedFieldsAcrossThreads {
    static class Codec {
        final Class<?> type;
        final IntUnaryOperator step;
        final String name;
        final String label;
        int uses;

        Codec(Class<?> type, IntUnaryOperator step, String name) {
            this.type = type;
            this.step = step;
            String first = name + "?";
            this.name = first.substring(0, first.length() - 1);
            this.label = describe();
            this.uses = this.name.length() + this.step.applyAsInt(0);
        }

        String describe() {
            return name + ":" + type.getSimpleName();
        }

        int apply(int x) {
            return step.applyAsInt(x) + name.length();
        }

        synchronized void use() {
            uses++;
        }
    }

    static class Counted extends Codec {
        final int[] seen = new int[1];

        Counted(String name) {
            super(Integer.class, x -> x * 3, name);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Codec[] codecs = {
            new Codec(String.class, x -> x + 1, "text"),
            new Counted("number"),
            new Codec(Codec.class, x -> x ^ 5, "self"),
        };
        long[] totals = new long[4];
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            int id = t;
            threads[t] = new Thread(() -> {
                long total = 0;
                for (int i = 0; i < 20000; i++) {
                    Codec c = codecs[(i + id) % codecs.length];
                    total += c.apply(i) + c.type.getName().length() + c.label.length();
                    if (i % 1000 == 0) {
                        c.use();
                    }
                }
                totals[id] = total;
            });
            threads[t].start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        for (int t = 0; t < totals.length; t++) {
            System.out.println("thread " + t + ": " + totals[t]);
        }
        for (Codec c : codecs) {
            System.out.println(c.describe() + " " + c.label + " " + c.uses + " " + (c.type == Integer.class));
        }
        Counted counted = (Counted) codecs[1];
        counted.seen[0] = counted.apply(2);
        System.out.println(counted.seen[0]);
    }
}
