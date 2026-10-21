// Synchronized methods of a base class run on objects of its subclasses, which threads share
// through a variable of the base type, and a subclass's own synchronized method calls the
// inherited one, which takes the same monitor again.
public class SynchronizedSubclasses {
    abstract static class Tally {
        private int count;

        synchronized void bump() {
            count++;
        }

        synchronized int count() {
            return count;
        }

        abstract String label();
    }

    static class Plain extends Tally {
        String label() {
            return "plain";
        }
    }

    static class Twice extends Tally {
        private int extra;

        synchronized void bumpTwice() {
            bump();
            bump();
            extra++;
        }

        synchronized int extra() {
            return extra;
        }

        String label() {
            return "twice";
        }
    }

    static class Worker extends Thread {
        private final Tally tally;

        Worker(Tally tally) {
            this.tally = tally;
        }

        @Override
        public void run() {
            for (int i = 0; i < 10000; i++) {
                if (tally instanceof Twice t && i % 2 == 0) {
                    t.bumpTwice();
                } else {
                    tally.bump();
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Tally[] tallies = {new Plain(), new Twice()};
        Worker[] workers = new Worker[4];
        for (int i = 0; i < workers.length; i++) {
            workers[i] = new Worker(tallies[i % 2]);
            workers[i].start();
        }
        for (Worker w : workers) {
            w.join();
        }
        for (Tally t : tallies) {
            System.out.println(t.label() + " " + t.count());
        }
        System.out.println("extra " + ((Twice) tallies[1]).extra());
    }
}
