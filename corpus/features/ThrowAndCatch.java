// Throwing and catching: a checked exception of the program with a field of its own, standard
// unchecked exceptions, catch clauses in order, a multi-catch, a try in a loop, an exception
// created without being thrown, and the text of exceptions with and without a message.
public class ThrowAndCatch {
    static class InsufficientFunds extends Exception {
        private final int missing;

        InsufficientFunds(String message, int missing) {
            super(message);
            this.missing = missing;
        }

        int getMissing() {
            return missing;
        }
    }

    static int balance = 100;

    static void withdraw(int amount) throws InsufficientFunds {
        if (amount < 0) {
            throw new IllegalArgumentException("negative amount: " + amount);
        }
        if (amount > balance) {
            throw new InsufficientFunds("cannot withdraw " + amount, amount - balance);
        }
        balance -= amount;
    }

    static int parseAmount(String text) {
        if (text.isEmpty()) {
            throw new IllegalStateException("empty");
        }
        return text.length() * 10;
    }

    static String attempt(int amount) {
        try {
            withdraw(amount);
            return "ok, balance " + balance;
        } catch (InsufficientFunds e) {
            return "failed: " + e.getMessage() + ", missing " + e.getMissing();
        } catch (IllegalArgumentException e) {
            return "bad: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(attempt(30));
        System.out.println(attempt(200));
        System.out.println(attempt(-5));
        int total = 0;
        for (int i = 0; i < 3; i++) {
            try {
                if (i == 1) {
                    throw new RuntimeException("skip " + i);
                }
                total += i;
            } catch (RuntimeException e) {
                System.out.println("caught " + e);
            }
        }
        System.out.println("total " + total);
        try {
            parseAmount("");
        } catch (IllegalStateException | IllegalArgumentException e) {
            System.out.println("multi " + e.getMessage());
        }
        System.out.println(parseAmount("abc"));
        Exception plain = new Exception("plain");
        System.out.println(plain);
        System.out.println(plain.getMessage());
        try {
            throw new UnsupportedOperationException();
        } catch (Exception e) {
            System.out.println(e + " " + (e.getMessage() == null));
        }
    }
}
