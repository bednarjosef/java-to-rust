// Objects that remember the thread that made them and check Thread.currentThread() against it
// on every use, from the main thread, from workers, and from a subclass of Thread.
public class CurrentThreadOwners {
    static final class Owned {
        final Thread owner;
        int uses;

        Owned() {
            this.owner = Thread.currentThread();
        }

        boolean use() {
            if (Thread.currentThread() != owner) {
                return false;
            }
            uses++;
            return true;
        }
    }

    static final class Counter extends Thread {
        final Owned mine = new Owned();
        Owned made;
        int refused;

        Counter(String name) {
            super(name);
        }

        @Override
        public void run() {
            made = new Owned();
            for (int i = 0; i < 10000; i++) {
                made.use();
                if (!mine.use()) {
                    refused++;
                }
            }
            if (Thread.currentThread() != this) {
                throw new IllegalStateException("not this thread");
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Owned main = new Owned();
        System.out.println("main owns: " + main.use() + " " + Thread.currentThread().getName());
        Counter[] counters = new Counter[3];
        for (int i = 0; i < 3; i++) {
            counters[i] = new Counter("counter-" + i);
        }
        for (Counter c : counters) {
            c.start();
        }
        boolean[] seen = new boolean[1];
        Thread plain = new Thread(() -> seen[0] = main.use() || Thread.currentThread().getName().equals("plain"), "plain");
        plain.start();
        plain.join();
        for (Counter c : counters) {
            c.join();
            System.out.println(c.getName() + ": made " + c.made.uses + ", refused " + c.refused + ", mine " + c.mine.uses
                    + ", owner " + (c.made.owner == c) + " " + (c.mine.owner == Thread.currentThread()));
        }
        System.out.println("plain: " + seen[0] + ", main uses " + main.uses);
    }
}
