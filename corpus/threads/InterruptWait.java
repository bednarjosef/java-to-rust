// interrupt ends a wait with InterruptedException after the monitor is taken again; a thread
// interrupted before it waits throws at once; IllegalMonitorStateException comes before the
// interrupt; a consumer stopped by an interrupt has taken every item put before it.
public class InterruptWait {
    static class Queue {
        private final int[] items = new int[4];
        private int head;
        private int size;

        synchronized void put(int v) throws InterruptedException {
            while (size == items.length) {
                wait();
            }
            items[(head + size) % items.length] = v;
            size++;
            notifyAll();
        }

        synchronized int take() throws InterruptedException {
            while (size == 0) {
                wait();
            }
            int v = items[head];
            head = (head + 1) % items.length;
            size--;
            notifyAll();
            return v;
        }
    }

    static class Consumer extends Thread {
        private final Queue queue;
        long sum;
        int taken;
        String end = "running";

        Consumer(Queue queue) {
            this.queue = queue;
        }

        @Override
        public void run() {
            try {
                while (true) {
                    sum += queue.take();
                    taken++;
                }
            } catch (InterruptedException e) {
                end = "stopped by " + e.getClass().getName() + ", message " + e.getMessage();
            }
        }
    }

    static class Waiter extends Thread {
        private final Object lock;
        String result = "not run";

        Waiter(Object lock) {
            this.lock = lock;
        }

        @Override
        public void run() {
            synchronized (lock) {
                try {
                    while (true) {
                        lock.wait();
                    }
                } catch (InterruptedException e) {
                    result = e + ", holds the lock: " + Thread.holdsLock(lock) + ", still interrupted: "
                            + isInterrupted();
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Object lock = new Object();
        Waiter waiter = new Waiter(lock);
        waiter.start();
        waiter.interrupt();
        waiter.join();
        System.out.println(waiter.result);

        Thread.currentThread().interrupt();
        synchronized (lock) {
            try {
                lock.wait(10);
                System.out.println("waited");
            } catch (InterruptedException e) {
                System.out.println("interrupted before a timed wait: " + e.getMessage());
            }
        }

        Thread.currentThread().interrupt();
        try {
            lock.wait();
        } catch (IllegalMonitorStateException e) {
            System.out.println("without the monitor: " + e.getMessage());
        }
        System.out.println("status kept: " + Thread.interrupted());

        Queue queue = new Queue();
        Consumer consumer = new Consumer(queue);
        consumer.start();
        long expected = 0;
        for (int i = 1; i <= 1000; i++) {
            queue.put(i);
            expected += i;
        }
        synchronized (queue) {
            while (queue.size > 0) {
                queue.wait(1);
            }
        }
        consumer.interrupt();
        consumer.join();
        System.out.println(consumer.end);
        System.out.println("taken " + consumer.taken + ", sum " + consumer.sum + ", expected " + expected);
    }
}
