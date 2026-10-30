// Waits on and notifies objects that the code reaches through an interface, `Object`, a field
// that a constructor's parameter sets and a static method's parameter, and no other way, so the
// analysis of which objects keep a monitor has to follow each to the class that made it; and
// threads share the objects of a class that no monitor site locks, which keep none.
public class WaitThroughInterface {
    interface Signal {
        void fire();

        boolean fired();
    }

    static class Flag implements Signal {
        private boolean set;

        public synchronized void fire() {
            set = true;
            notifyAll();
        }

        public synchronized boolean fired() {
            return set;
        }
    }

    static class Slot {
        int value;
    }

    static class Holder {
        final Object lock;

        Holder(Object lock) {
            this.lock = lock;
        }
    }

    static class Hits {
        volatile int count;
    }

    static final Hits hits = new Hits();

    static void putAndNotify(Object lock, int value) {
        synchronized (lock) {
            ((Slot) lock).value = value;
            lock.notifyAll();
        }
    }

    static int waitFor(Object lock) throws InterruptedException {
        synchronized (lock) {
            while (((Slot) lock).value == 0) {
                lock.wait();
            }
            return ((Slot) lock).value;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        // Through an interface, waited on as an `Object`.
        Signal signal = new Flag();
        Thread firing = new Thread(() -> {
            hits.count++;
            signal.fire();
        });
        firing.start();
        Object asObject = signal;
        synchronized (asObject) {
            while (!signal.fired()) {
                asObject.wait();
            }
        }
        firing.join();
        System.out.println("signal fired: " + signal.fired());

        // A field of type `Object` that a constructor's parameter sets.
        Holder holder = new Holder(new Slot());
        Thread putting = new Thread(() -> {
            hits.count++;
            putAndNotify(holder.lock, 7);
        });
        putting.start();
        System.out.println("through a field: " + waitFor(holder.lock));
        putting.join();

        System.out.println("threads that ran: " + hits.count);
    }
}
