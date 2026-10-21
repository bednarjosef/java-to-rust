// Threads that run a Runnable: a class that implements it, and a lambda that captures arrays.
public class RunnableTasks {
    static class Counter implements Runnable {
        private final int[] slots;
        private final int index;

        Counter(int[] slots, int index) {
            this.slots = slots;
            this.index = index;
        }

        public void run() {
            for (int i = 0; i < 1000; i++) {
                slots[index] += i % (index + 2);
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int[] slots = new int[3];
        Thread[] threads = new Thread[3];
        for (int i = 0; i < 3; i++) {
            threads[i] = new Thread(new Counter(slots, i), "counter-" + i);
            threads[i].start();
        }
        for (Thread t : threads) {
            t.join();
            System.out.println(t.getName() + " done");
        }
        int[] total = new int[1];
        Thread lambda = new Thread(() -> {
            for (int s : slots) {
                total[0] += s;
            }
        });
        lambda.start();
        lambda.join();
        System.out.println("total " + total[0]);
    }
}
