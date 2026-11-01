// Math.random: a double in [0, 1) from one generator the whole program shares, also between
// threads. The numbers differ from run to run, so only their properties are printed.
public class MathRandom {
    public static void main(String[] args) throws InterruptedException {
        double[] numbers = new double[1000];
        for (int i = 0; i < numbers.length; i++) {
            numbers[i] = Math.random();
        }
        boolean inRange = true;
        int distinct = 0;
        double sum = 0;
        for (int i = 0; i < numbers.length; i++) {
            inRange &= numbers[i] >= 0.0 && numbers[i] < 1.0;
            if (i == 0 || numbers[i] != numbers[i - 1]) {
                distinct++;
            }
            sum += numbers[i];
        }
        System.out.println("in [0, 1): " + inRange);
        System.out.println("varies: " + (distinct > 900));
        System.out.println("mean near 0.5: " + (Math.abs(sum / numbers.length - 0.5) < 0.1));
        int[] low = new int[1];
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            threads[t] = new Thread(() -> {
                for (int i = 0; i < 1000; i++) {
                    double x = Math.random();
                    if (x < 0.0 || x >= 1.0) {
                        synchronized (low) {
                            low[0]++;
                        }
                    }
                }
            });
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        System.out.println("out of range from threads: " + low[0]);
        int dice = (int) (Math.random() * 6) + 1;
        System.out.println("die in 1..6: " + (dice >= 1 && dice <= 6));
    }
}
