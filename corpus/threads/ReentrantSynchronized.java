// Synchronized methods that call each other on the same object, and a synchronized block on
// another object inside one: Java's monitors are reentrant.
public class ReentrantSynchronized {
    static class Account {
        private long balance;
        private int operations;

        synchronized void deposit(long amount) {
            balance += amount;
            record();
        }

        synchronized void record() {
            operations++;
        }

        synchronized void transferTo(Account other, long amount) {
            balance -= amount;
            record();
            synchronized (other) {
                other.deposit(amount);
            }
        }

        synchronized long balance() {
            return balance;
        }

        synchronized int operations() {
            return operations;
        }
    }

    static class Mover extends Thread {
        private final Account from;
        private final Account to;

        Mover(Account from, Account to) {
            this.from = from;
            this.to = to;
        }

        @Override
        public void run() {
            for (int i = 0; i < 5000; i++) {
                from.deposit(2);
                from.transferTo(to, 1);
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Account a = new Account();
        Account b = new Account();
        Mover first = new Mover(a, b);
        Mover second = new Mover(a, b);
        first.start();
        second.start();
        first.join();
        second.join();
        System.out.println("a " + a.balance() + " in " + a.operations() + " operations");
        System.out.println("b " + b.balance() + " in " + b.operations() + " operations");
    }
}
