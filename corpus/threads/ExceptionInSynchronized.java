// An exception that leaves a synchronized block or method exits the monitor, so another thread
// can enter it afterwards; a lock held in a field guards the object's state.
public class ExceptionInSynchronized {
    static class Ledger {
        private final Object lock = new Object();
        private int entries;

        void record(int amount) {
            synchronized (lock) {
                if (amount < 0) {
                    throw new IllegalArgumentException("negative amount " + amount);
                }
                entries++;
            }
        }

        synchronized void check(int value) {
            if (value % 1000 == 999) {
                throw new IllegalStateException("bad value " + value);
            }
        }

        int entries() {
            synchronized (lock) {
                return entries;
            }
        }
    }

    static class Recorder extends Thread {
        private final Ledger ledger;
        int rejected;

        Recorder(Ledger ledger) {
            this.ledger = ledger;
        }

        @Override
        public void run() {
            for (int i = 0; i < 5000; i++) {
                try {
                    ledger.record(i % 7 == 0 ? -i : i);
                    ledger.check(i);
                } catch (IllegalArgumentException | IllegalStateException e) {
                    rejected++;
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Ledger ledger = new Ledger();
        Recorder a = new Recorder(ledger);
        Recorder b = new Recorder(ledger);
        a.start();
        b.start();
        a.join();
        b.join();
        System.out.println("entries " + ledger.entries() + ", rejected " + (a.rejected + b.rejected));
        try {
            ledger.record(-1);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        ledger.record(1);
        System.out.println("entries " + ledger.entries());
    }
}
