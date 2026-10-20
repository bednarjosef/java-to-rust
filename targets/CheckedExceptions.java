// Features: custom checked exceptions (extends Exception) with a cause, custom unchecked exceptions
// (extends RuntimeException), throws clauses, try/catch/finally with finally ordering, multi-catch,
// rethrow, getCause, exception chaining, nested try blocks, return from finally overriding a
// return value, a finally block that runs on break, and catching a built-in NumberFormatException.
public class CheckedExceptions {
    static class InsufficientFundsException extends Exception {
        private final int shortfall;

        InsufficientFundsException(int shortfall) {
            super("short by " + shortfall);
            this.shortfall = shortfall;
        }

        int getShortfall() {
            return shortfall;
        }
    }

    static class AccountLockedException extends RuntimeException {
        AccountLockedException(String account, Throwable cause) {
            super("account " + account + " is locked", cause);
        }
    }

    static class Account {
        private final String id;
        private int balance;
        private int failures;

        Account(String id, int balance) {
            this.id = id;
            this.balance = balance;
        }

        void withdraw(int amount) throws InsufficientFundsException {
            if (amount <= 0) {
                throw new IllegalArgumentException("amount must be positive: " + amount);
            }
            if (amount > balance) {
                failures++;
                if (failures >= 2) {
                    throw new AccountLockedException(id, new IllegalStateException("too many failures"));
                }
                throw new InsufficientFundsException(amount - balance);
            }
            balance -= amount;
        }

        int getBalance() {
            return balance;
        }
    }

    static int parseOrDefault(String text, int fallback) {
        try {
            return Integer.parseInt(text);
        } catch (NumberFormatException e) {
            System.out.println("  bad number '" + text + "'");
            return fallback;
        } finally {
            System.out.println("  parsed " + text);
        }
    }

    @SuppressWarnings("finally")
    static int finallyWins() {
        try {
            return 1;
        } finally {
            System.out.println("  finally runs before return");
        }
    }

    static int returnFromFinally() {
        try {
            throw new RuntimeException("lost");
        } finally {
            return 42;
        }
    }

    static void attempt(Account account, int amount) {
        System.out.println("withdraw " + amount + " from " + account.id);
        try {
            account.withdraw(amount);
            System.out.println("  ok, balance " + account.getBalance());
        } catch (InsufficientFundsException e) {
            System.out.println("  insufficient: " + e.getMessage() + " (" + e.getShortfall() + ")");
        } catch (IllegalArgumentException | IllegalStateException e) {
            System.out.println("  rejected: " + e.getMessage());
        } catch (RuntimeException e) {
            System.out.println("  " + e.getMessage() + ", caused by " + e.getCause().getMessage());
        } finally {
            System.out.println("  end of attempt");
        }
    }

    static void chain(int level) throws Exception {
        try {
            if (level == 0) {
                throw new java.io.IOException("disk gone");
            }
            chain(level - 1);
        } catch (java.io.IOException e) {
            throw new Exception("level " + level + " failed", e);
        }
    }

    public static void main(String[] args) throws Exception {
        Account account = new Account("acc-1", 100);
        attempt(account, 30);
        attempt(account, 500);
        attempt(account, -5);
        attempt(account, 900);
        attempt(account, 10);

        System.out.println("parse results: " + parseOrDefault("17", 0) + " " + parseOrDefault("x1", -1));
        System.out.println("finallyWins: " + finallyWins());
        System.out.println("returnFromFinally: " + returnFromFinally());

        for (int i = 0; i < 3; i++) {
            try {
                if (i == 1) {
                    break;
                }
                System.out.println("loop body " + i);
            } finally {
                System.out.println("loop finally " + i);
            }
        }

        try {
            chain(2);
        } catch (Exception e) {
            System.out.println(e.getMessage());
            Throwable cause = e;
            int depth = 0;
            while (cause.getCause() != null) {
                cause = cause.getCause();
                depth++;
            }
            System.out.println("root cause: " + cause.getClass().getSimpleName() + " after " + depth + " causes");
        }

        try {
            try {
                throw new IllegalStateException("inner");
            } finally {
                System.out.println("inner finally");
            }
        } catch (IllegalStateException e) {
            System.out.println("outer caught " + e.getMessage());
        }
        System.out.println("balance at end: " + account.getBalance());
    }
}
