// Waits on and notifies objects that the code reaches through an array of `Object`s and the
// result of a call through an interface, which the analysis of which objects keep a monitor does
// not follow: every class whose objects become `Object`s keeps its monitor.
public class WaitThroughArraysAndCalls {
    static class Slot {
        int value;
    }

    interface Source {
        Object get();
    }

    static class Fixed implements Source {
        private final Object held;

        Fixed(Object held) {
            this.held = held;
        }

        public Object get() {
            return held;
        }
    }

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
        Object[] locks = {new Slot(), new Slot()};
        Thread second = new Thread(() -> putAndNotify(locks[1], 11));
        second.start();
        System.out.println("through an array: " + waitFor(locks[1]));
        second.join();

        Source source = new Fixed(new Slot());
        Thread third = new Thread(() -> putAndNotify(source.get(), 13));
        third.start();
        System.out.println("through a call: " + waitFor(source.get()));
        third.join();
    }
}
