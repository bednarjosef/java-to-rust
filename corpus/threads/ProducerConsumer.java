// A bounded buffer with wait and notifyAll: a producer puts numbers in, a consumer takes them
// out in order and sums them.
public class ProducerConsumer {
    static class Buffer {
        private final int[] items = new int[4];
        private int head;
        private int count;

        synchronized void put(int value) throws InterruptedException {
            while (count == items.length) {
                wait();
            }
            items[(head + count) % items.length] = value;
            count++;
            notifyAll();
        }

        synchronized int take() throws InterruptedException {
            while (count == 0) {
                wait();
            }
            int value = items[head];
            head = (head + 1) % items.length;
            count--;
            notifyAll();
            return value;
        }
    }

    static class Producer extends Thread {
        private final Buffer buffer;

        Producer(Buffer buffer) {
            this.buffer = buffer;
        }

        @Override
        public void run() {
            try {
                for (int i = 1; i <= 1000; i++) {
                    buffer.put(i);
                }
                buffer.put(-1);
            } catch (InterruptedException e) {
                System.out.println("producer interrupted");
            }
        }
    }

    static class Consumer extends Thread {
        private final Buffer buffer;
        long sum;
        int taken;
        boolean ordered = true;

        Consumer(Buffer buffer) {
            this.buffer = buffer;
        }

        @Override
        public void run() {
            try {
                int last = 0;
                while (true) {
                    int value = buffer.take();
                    if (value < 0) {
                        return;
                    }
                    if (value != last + 1) {
                        ordered = false;
                    }
                    last = value;
                    sum += value;
                    taken++;
                }
            } catch (InterruptedException e) {
                System.out.println("consumer interrupted");
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Buffer buffer = new Buffer();
        Producer producer = new Producer(buffer);
        Consumer consumer = new Consumer(buffer);
        consumer.start();
        producer.start();
        producer.join();
        consumer.join();
        System.out.println("taken " + consumer.taken + ", sum " + consumer.sum + ", in order " + consumer.ordered);
    }
}
