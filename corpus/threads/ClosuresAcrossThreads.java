// Functional interfaces whose values are only lambdas, reaching other threads: one captured by a
// lambda that a thread runs, one held in a field of a thread, and one that a thread creates and
// hands back.
public class ClosuresAcrossThreads {
    interface Step {
        int apply(int x);
    }

    interface Combine {
        long of(long a, int b);
    }

    static class Folder extends Thread {
        private final int[] values;
        private final Combine combine;
        long result;
        Step made;

        Folder(int[] values, Combine combine) {
            this.values = values;
            this.combine = combine;
        }

        @Override
        public void run() {
            long acc = 0;
            for (int v : values) {
                acc = combine.of(acc, v);
            }
            result = acc;
            int bias = (int) (acc % 7);
            made = x -> x * 2 + bias;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int offset = 3;
        Step shift = x -> x + offset;
        Step square = x -> x * x;
        int[] out = new int[4];
        Thread[] workers = new Thread[2];
        for (int w = 0; w < 2; w++) {
            Step step = w == 0 ? shift : square;
            int base = w * 2;
            workers[w] = new Thread(() -> {
                for (int i = 0; i < 2; i++) {
                    out[base + i] = step.apply(base + i + 1);
                }
            });
            workers[w].start();
        }
        for (Thread t : workers) {
            t.join();
        }
        for (int v : out) {
            System.out.println(v);
        }

        int[] values = {5, 8, 13, 21};
        Folder sum = new Folder(values, (a, b) -> a + b);
        Folder weighted = new Folder(values, (a, b) -> a * 3 + b);
        sum.start();
        weighted.start();
        sum.join();
        weighted.join();
        System.out.println(sum.result + " " + weighted.result);
        System.out.println(sum.made.apply(10) + " " + weighted.made.apply(10));
    }
}
