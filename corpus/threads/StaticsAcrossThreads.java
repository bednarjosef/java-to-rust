// Statics are the process's: workers read a static that main set before starting them, and write
// their results into a static array.
public class StaticsAcrossThreads {
    static int base;
    static long[] results = new long[3];
    static String label = "sum";

    static class Adder extends Thread {
        private final int index;

        Adder(int index) {
            this.index = index;
        }

        @Override
        public void run() {
            long total = 0;
            for (int i = 0; i < 1000; i++) {
                total += base + i * index;
            }
            results[index] = total;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        base = 7;
        Adder[] adders = new Adder[3];
        for (int i = 0; i < adders.length; i++) {
            adders[i] = new Adder(i);
        }
        for (Adder a : adders) {
            a.start();
        }
        for (Adder a : adders) {
            a.join();
        }
        for (int i = 0; i < results.length; i++) {
            System.out.println(label + " " + i + " = " + results[i]);
        }
    }
}
