// Each worker fills its own slice of one array that every worker holds; main prints the array
// once all of them are done.
public class SharedArraySlices {
    static class Filler extends Thread {
        private final long[] out;
        private final int from;
        private final int to;

        Filler(long[] out, int from, int to) {
            this.out = out;
            this.from = from;
            this.to = to;
        }

        @Override
        public void run() {
            for (int i = from; i < to; i++) {
                long v = 1;
                for (int k = 0; k < i % 7; k++) {
                    v = v * 3 + i;
                }
                out[i] = v;
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int n = 40;
        long[] values = new long[n];
        Filler[] fillers = new Filler[4];
        int step = n / fillers.length;
        for (int w = 0; w < fillers.length; w++) {
            fillers[w] = new Filler(values, w * step, w == fillers.length - 1 ? n : (w + 1) * step);
            fillers[w].start();
        }
        for (Filler f : fillers) {
            f.join();
        }
        long sum = 0;
        StringBuilder line = new StringBuilder();
        for (int i = 0; i < n; i++) {
            sum += values[i];
            if (i < 10) {
                line.append(values[i]).append(' ');
            }
        }
        System.out.println(line.toString().trim());
        System.out.println("sum " + sum);
    }
}
