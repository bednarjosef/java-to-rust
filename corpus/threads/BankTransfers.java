// Threads move money between accounts, each transfer holding both accounts' monitors, taken in
// the order of the accounts' numbers so that no two transfers wait for each other. The total is
// kept, and each account ends with the same balance in every run.
public class BankTransfers {
    static class Account {
        final int number;
        private long balance;

        Account(int number, long balance) {
            this.number = number;
            this.balance = balance;
        }

        long balance() {
            synchronized (this) {
                return balance;
            }
        }

        void add(long amount) {
            balance += amount;
        }
    }

    static void transfer(Account from, Account to, long amount) {
        Account first = from.number < to.number ? from : to;
        Account second = first == from ? to : from;
        synchronized (first) {
            synchronized (second) {
                from.add(-amount);
                to.add(amount);
            }
        }
    }

    static class Clerk extends Thread {
        private final Account[] accounts;
        private final int seed;

        Clerk(Account[] accounts, int seed) {
            this.accounts = accounts;
            this.seed = seed;
        }

        @Override
        public void run() {
            int x = seed;
            for (int i = 0; i < 20000; i++) {
                x = x * 1103515245 + 12345;
                int from = (x >>> 8) % accounts.length;
                int to = (x >>> 16) % accounts.length;
                if (from != to) {
                    transfer(accounts[from], accounts[to], (x >>> 4) % 50);
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Account[] accounts = new Account[5];
        for (int i = 0; i < accounts.length; i++) {
            accounts[i] = new Account(i, 1000);
        }
        Clerk[] clerks = new Clerk[3];
        for (int i = 0; i < clerks.length; i++) {
            clerks[i] = new Clerk(accounts, i + 1);
            clerks[i].start();
        }
        for (Clerk c : clerks) {
            c.join();
        }
        long total = 0;
        for (Account a : accounts) {
            System.out.println("account " + a.number + ": " + a.balance());
            total += a.balance();
        }
        System.out.println("total " + total);
    }
}
