public class BankQueueSimulation {
    /** A fixed-capacity FIFO of customer arrival times, stored in a ring. */
    static class RingBuffer {
        private final int[] items;
        private int head;
        private int size;

        RingBuffer(int capacity) {
            items = new int[capacity];
        }

        boolean isEmpty() {
            return size == 0;
        }

        boolean isFull() {
            return size == items.length;
        }

        int size() {
            return size;
        }

        boolean offer(int value) {
            if (isFull()) {
                return false;
            }
            items[(head + size) % items.length] = value;
            size++;
            return true;
        }

        /** Returns the oldest value, or -1 when the queue is empty. */
        int poll() {
            if (isEmpty()) {
                return -1;
            }
            int value = items[head];
            head = (head + 1) % items.length;
            size--;
            return value;
        }
    }

    static class Teller {
        int busyUntil;
        int served;
        int totalBusy;

        boolean free(int now) {
            return busyUntil <= now;
        }

        void serve(int now, int duration) {
            busyUntil = now + duration;
            served++;
            totalBusy += duration;
        }
    }

    static class Stats {
        int arrivals;
        int rejected;
        long totalWait;
        int maxWait;
        int maxQueue;
    }

    static int seed = 2718;

    /** Linear congruential generator returning values in [0, bound). */
    static int nextRandom(int bound) {
        seed = seed * 1103515245 + 12345;
        return ((seed >>> 16) & 0x7fff) % bound;
    }

    static void simulate(int tellerCount, int capacity, int minutes, Stats stats) {
        RingBuffer queue = new RingBuffer(capacity);
        Teller[] tellers = new Teller[tellerCount];
        for (int i = 0; i < tellerCount; i++) {
            tellers[i] = new Teller();
        }
        for (int now = 0; now < minutes; now++) {
            if (nextRandom(10) < 4) {
                stats.arrivals++;
                if (!queue.offer(now)) {
                    stats.rejected++;
                }
            }
            for (Teller t : tellers) {
                if (t.free(now) && !queue.isEmpty()) {
                    int arrived = queue.poll();
                    int wait = now - arrived;
                    stats.totalWait += wait;
                    stats.maxWait = Math.max(stats.maxWait, wait);
                    t.serve(now, 3 + nextRandom(6));
                }
            }
            stats.maxQueue = Math.max(stats.maxQueue, queue.size());
        }
    }

    static void report(int tellerCount, int capacity, Stats s) {
        int served = s.arrivals - s.rejected;
        System.out.println(tellerCount + " tellers, queue " + capacity + ": arrivals " + s.arrivals
                + ", served-or-waiting " + served + ", rejected " + s.rejected);
        System.out.println("  max wait " + s.maxWait + ", max queue " + s.maxQueue);
    }

    public static void main(String[] args) {
        RingBuffer ring = new RingBuffer(3);
        for (int v = 1; v <= 5; v++) {
            System.out.println("offer " + v + ": " + ring.offer(v) + " size " + ring.size());
        }
        System.out.println("poll " + ring.poll());
        System.out.println("offer 6: " + ring.offer(6));
        while (!ring.isEmpty()) {
            System.out.println("drain " + ring.poll());
        }

        int[][] configs = {{1, 5}, {2, 5}, {2, 20}, {3, 8}};
        for (int[] cfg : configs) {
            Stats stats = new Stats();
            seed = 2718;
            simulate(cfg[0], cfg[1], 600, stats);
            report(cfg[0], cfg[1], stats);
            System.out.println("  average wait " + (stats.arrivals == 0 ? 0 : stats.totalWait * 100 / stats.arrivals) / 100.0);
        }

        int[] total = new int[3];
        Teller[] tellers = new Teller[3];
        for (int i = 0; i < tellers.length; i++) {
            tellers[i] = new Teller();
        }
        for (int now = 0; now < 40; now++) {
            for (Teller t : tellers) {
                if (t.free(now)) {
                    t.serve(now, 2);
                }
            }
        }
        for (int i = 0; i < tellers.length; i++) {
            total[i] = tellers[i].served;
        }
        System.out.println("served per teller: " + total[0] + " " + total[1] + " " + total[2]);

        System.out.println("poll on empty: " + ring.poll());
    }
}
