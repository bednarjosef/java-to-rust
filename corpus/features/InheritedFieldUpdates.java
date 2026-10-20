public class InheritedFieldUpdates {
    static class Account {
        String owner;
        int balance;
        String log = "";
        Account(String owner) { this.owner = owner; }
        int fee() { return 1; }
        void charge() { balance -= fee(); log += "c"; }
        String summary() { return owner + ":" + balance + ":" + log; }
        int bump() { balance += 10; return balance; }
    }

    static class Premium extends Account {
        int level = 2;
        String title = owner + "*";
        Premium(String owner) { super(owner); }
        int fee() { return 0; }
        String summary() { return title + " " + super.summary(); }
    }

    static class Student extends Account {
        Student(String owner) { super(owner); }
        int fee() { return balance > 50 ? 1 : 0; }
    }

    static Account pick(boolean premium, String name) {
        return premium ? new Premium(name) : new Student(name);
    }

    static Account richer(Account a, Account b) {
        if (a == null) return b;
        if (b == null) return a;
        return a.balance >= b.balance ? a : b;
    }

    static String kind(Account a) {
        switch (a.fee()) {
            case 0: return "free";
            case 1: return "paid";
            default: return "?";
        }
    }

    public static void main(String[] args) {
        Account plain = new Account("ann");
        Account prem = pick(true, "bob");
        Account stud = pick(false, "cy");
        Account[] all = { plain, prem, stud };
        for (Account a : all) {
            a.balance += 100;
            a.charge();
            a.owner += "!";
            a.log += a.fee();
        }
        for (Account a : all) System.out.println(a.summary() + " " + kind(a));
        System.out.println(plain.bump() + " " + prem.bump() + " " + (stud.balance++) + " " + (++stud.balance));
        Account best = null;
        for (Account a : all) best = richer(best, a);
        System.out.println(best.summary());
        System.out.println(richer(null, null) == null);
        best.balance = best.balance + best.bump();
        System.out.println(best.balance);
        Premium p = new Premium("dee");
        p.level++;
        Account asAccount = p;
        asAccount.charge();
        System.out.println(p.summary() + " " + p.level + " " + asAccount.summary());
        Account none = null;
        System.out.println("none: " + (none == null) + " " + richer(none, plain).owner);
    }
}
