class Account {
    int balance;
    int transactions;

    Account(int opening) {
        balance = opening;
    }

    void deposit(int amount) {
        balance += amount;
        transactions++;
    }

    boolean withdraw(int amount) {
        if (amount > balance) {
            return false;
        }
        balance -= amount;
        transactions++;
        return true;
    }

    int fee() {
        return balance / 100;
    }

    public String toString() {
        return "balance " + balance + " after " + transactions + " transactions";
    }
}

public class BankAccounts {
    public static void main(String[] args) {
        Account checking = new Account(100);
        Account savings = new Account(0);
        checking.deposit(50);
        if (checking.withdraw(120)) {
            System.out.println("paid 120");
        }
        if (!savings.withdraw(500)) {
            System.out.println("declined 500");
        }
        savings.deposit(checking.balance / 2);
        checking.withdraw(checking.balance);
        System.out.println(checking);
        System.out.println(savings);
        savings.deposit(savings.fee() * 100 + 7);
        System.out.println(savings.balance + " " + savings.fee());
        int total = checking.balance + savings.balance;
        System.out.println("total " + total);
        System.out.println(checking.transactions + savings.transactions);
    }
}
