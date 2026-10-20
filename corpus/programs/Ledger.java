// A small ledger: accounts with a common abstract superclass that keeps the balance and the
// history, and a report built as a template method whose steps a subclass fills in.
public class Ledger {
    abstract static class Account {
        private static int opened = 0;
        private final int number;
        private final String holder;
        protected long cents;
        private long[] history = new long[8];
        private int entries;

        protected Account(String holder, long cents) {
            opened++;
            this.number = 1000 + opened;
            this.holder = holder;
            this.cents = cents;
            record(cents);
        }

        int number() {
            return number;
        }

        String holder() {
            return holder;
        }

        long balance() {
            return cents;
        }

        protected void record(long amount) {
            if (entries == history.length) {
                long[] bigger = new long[history.length * 2];
                for (int i = 0; i < history.length; i++) {
                    bigger[i] = history[i];
                }
                history = bigger;
            }
            history[entries] = amount;
            entries++;
        }

        int entries() {
            return entries;
        }

        long entry(int i) {
            return history[i];
        }

        boolean deposit(long amount) {
            if (amount <= 0) {
                return false;
            }
            cents += amount;
            record(amount);
            return true;
        }

        boolean withdraw(long amount) {
            if (amount > available()) {
                return false;
            }
            cents -= amount;
            record(-amount);
            return true;
        }

        abstract long available();

        abstract String kind();

        void transferTo(Account other, long amount) {
            if (withdraw(amount)) {
                other.deposit(amount);
            }
        }

        static int opened() {
            return opened;
        }

        @Override
        public String toString() {
            return kind() + " " + number + " (" + holder + "): " + Report.money(cents);
        }
    }

    static class Checking extends Account {
        private final long overdraft;
        private int withdrawals;

        Checking(String holder, long cents, long overdraft) {
            super(holder, cents);
            this.overdraft = overdraft;
        }

        @Override
        long available() {
            return cents + overdraft;
        }

        @Override
        boolean withdraw(long amount) {
            boolean done = super.withdraw(amount);
            if (done) {
                withdrawals++;
            }
            return done;
        }

        @Override
        String kind() {
            return withdrawals > 2 ? "busy checking" : "checking";
        }
    }

    abstract static class Report {
        private String out = "";
        private int lines;

        static String money(long cents) {
            String sign = cents < 0 ? "-" : "";
            long abs = Math.abs(cents);
            long rest = abs % 100;
            return sign + (abs / 100) + "." + (rest < 10 ? "0" : "") + rest;
        }

        protected void line(String text) {
            lines++;
            out += lines + ". " + text + "\n";
        }

        abstract void header();

        abstract void body();

        String render() {
            header();
            body();
            line("end of report");
            return out;
        }
    }

    static class Statement extends Report {
        private final Checking account;

        Statement(Checking account) {
            this.account = account;
        }

        @Override
        void header() {
            line("statement for " + account.holder());
        }

        @Override
        void body() {
            long running = 0;
            for (int i = 0; i < account.entries(); i++) {
                running += account.entry(i);
                line(money(account.entry(i)) + " -> " + money(running));
            }
        }
    }

    public static void main(String[] args) {
        Checking alice = new Checking("alice", 12_000, 5_000);
        Checking bob = new Checking("bob", 300, 0);
        Account[] accounts = {alice, bob};
        alice.deposit(2_550);
        alice.transferTo(bob, 4_000);
        bob.transferTo(alice, 10_000);
        for (int i = 0; i < 7; i++) {
            alice.withdraw(1_234 + i);
        }
        if (!bob.deposit(-5)) {
            System.out.println("rejected: deposits must be positive");
        }
        long total = 0;
        for (Account a : accounts) {
            System.out.println(a.number() + ": " + a);
            total += a.balance();
        }
        System.out.println("total " + Report.money(total) + " in " + Account.opened() + " accounts");
        System.out.print(new Statement(alice).render());
        Report report = new Statement(bob);
        System.out.print(report.render());
    }
}
