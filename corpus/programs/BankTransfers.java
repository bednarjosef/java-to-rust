// A small bank: transfers between accounts fail with checked exceptions of the program (a
// subclass for a locked account) and with IllegalArgumentException, which the bank reports, and
// the last transfer fails with nothing to catch it, which ends the program.
public class BankTransfers {
    static class TransferException extends Exception {
        TransferException(String message) {
            super(message);
        }
    }

    static class AccountLocked extends TransferException {
        AccountLocked(String id) {
            super("account " + id + " is locked");
        }
    }

    static class Account {
        final String id;
        private long balance;
        private boolean locked;
        private int failures;

        Account(String id, long balance) {
            this.id = id;
            this.balance = balance;
        }

        void withdraw(long amount) throws TransferException {
            if (locked) {
                throw new AccountLocked(id);
            }
            if (amount <= 0) {
                throw new IllegalArgumentException("amount must be positive: " + amount);
            }
            if (amount > balance) {
                failures++;
                if (failures >= 2) {
                    locked = true;
                }
                throw new TransferException("insufficient funds in " + id + ": " + balance + " < " + amount);
            }
            balance -= amount;
        }

        void deposit(long amount) {
            if (amount <= 0) {
                throw new IllegalArgumentException("amount must be positive: " + amount);
            }
            balance += amount;
        }

        long balance() {
            return balance;
        }
    }

    static class Bank {
        private final Account[] accounts;
        private int completed;
        private String lastError = "none";

        Bank(Account[] accounts) {
            this.accounts = accounts;
        }

        Account find(String id) throws TransferException {
            for (Account a : accounts) {
                if (a.id.equals(id)) {
                    return a;
                }
            }
            throw new TransferException("no account " + id);
        }

        void transfer(String from, String to, long amount) throws TransferException {
            Account source = find(from);
            Account target = find(to);
            source.withdraw(amount);
            target.deposit(amount);
            completed++;
        }

        boolean tryTransfer(String from, String to, long amount) {
            try {
                transfer(from, to, amount);
                return true;
            } catch (AccountLocked e) {
                lastError = "locked: " + e.getMessage();
            } catch (TransferException e) {
                lastError = e.getMessage();
            } catch (IllegalArgumentException e) {
                lastError = "invalid: " + e.getMessage();
            }
            return false;
        }
    }

    public static void main(String[] args) throws TransferException {
        Account[] accounts = { new Account("alice", 100), new Account("bob", 50), new Account("carol", 0) };
        Bank bank = new Bank(accounts);
        String[][] requests = {
            { "alice", "bob", "30" },
            { "bob", "carol", "100" },
            { "bob", "carol", "90" },
            { "bob", "alice", "10" },
            { "dave", "alice", "5" },
            { "alice", "carol", "-4" },
            { "carol", "alice", "0" },
            { "alice", "carol", "70" },
        };
        for (String[] r : requests) {
            boolean ok = bank.tryTransfer(r[0], r[1], Long.parseLong(r[2]));
            System.out.println(r[0] + " -> " + r[1] + " " + r[2] + ": " + (ok ? "done" : "failed (" + bank.lastError + ")"));
        }
        for (Account a : accounts) {
            System.out.println(a.id + " " + a.balance());
        }
        System.out.println("completed " + bank.completed);
        int status;
        try {
            bank.transfer("carol", "bob", 1000);
            status = 0;
        } catch (TransferException e) {
            System.out.println(e);
            status = 2;
        }
        System.out.println("status " + status);
        bank.transfer("alice", "bob", 1);
    }
}
