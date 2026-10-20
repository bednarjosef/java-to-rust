// `super(...)` and `super.m(...)` through a chain of three classes: a superclass constructor that
// delegates with `this(...)`, overrides that call the method they override, a superclass's method
// that only `super` reaches, and methods of the superclass that use the object as a value.
public class SuperCalls {
    static int created = 0;

    abstract static class Account {
        private static int nextId = 100;
        protected final int id;
        protected long balance = 10;
        private String owner;
        private int log = 0;

        Account(String owner) {
            this(owner, 0);
        }

        Account(String owner, long initial) {
            this.id = nextId++;
            this.owner = owner;
            this.balance += initial;
            created++;
        }

        void deposit(long amount) {
            balance += amount;
            note();
        }

        void note() {
            log++;
        }

        String owner() {
            return owner;
        }

        abstract String kind();

        String summary() {
            return kind() + " #" + id + " of " + owner + ": " + balance;
        }

        @Override
        public String toString() {
            return "Account(" + owner + ", " + balance + ", log " + log + ")";
        }
    }

    abstract static class Savings extends Account {
        protected double rate;

        Savings(String owner, double rate) {
            super(owner, 5);
            this.rate = rate;
        }

        @Override
        void deposit(long amount) {
            super.deposit(amount + 1);
            balance += 1;
        }

        @Override
        String kind() {
            return "savings";
        }
    }

    static class Bonus extends Savings {
        private int bonuses;

        Bonus(String owner) {
            super(owner, 0.5);
        }

        @Override
        void deposit(long amount) {
            super.deposit(amount);
            bonuses++;
            if (bonuses % 2 == 0) {
                super.deposit(100);
            }
        }

        @Override
        String summary() {
            return "bonus " + super.summary() + " after " + bonuses;
        }

        Bonus self() {
            return this;
        }
    }

    static void report(Account a) {
        System.out.println(a.summary());
        System.out.println(a);
    }

    public static void main(String[] args) {
        Bonus b = new Bonus("ada");
        b.deposit(10);
        b.deposit(20);
        b.self().deposit(1);
        report(b);
        Account a = b;
        a.deposit(3);
        System.out.println(a.owner() + " " + a.kind() + " " + created);
        Savings s = b;
        System.out.println(s.rate + " " + (s == a));
        Account[] all = new Account[] {b, new Bonus("bob")};
        for (Account x : all) {
            System.out.println(x.summary());
        }
    }
}
