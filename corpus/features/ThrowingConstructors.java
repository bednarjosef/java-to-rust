// Constructors that throw: a check of the arguments before any field is set, a throw after some
// fields are set (the object is never seen), a constructor that delegates with `this(...)` to one
// that throws, a subclass whose `super(...)` throws, a constructor that calls a method that
// throws, and one whose division by zero is caught by the caller.
public class ThrowingConstructors {
    static int built = 0;

    static class Range {
        final int low;
        final int high;

        Range(int low, int high) {
            if (low > high) {
                throw new IllegalArgumentException("empty range " + low + ".." + high);
            }
            this.low = low;
            this.high = high;
            built++;
        }

        Range(int high) {
            this(0, high);
        }

        int size() {
            return high - low + 1;
        }
    }

    static class Account {
        final String owner;
        int balance;

        Account(String owner, int balance) {
            this.owner = owner;
            this.balance = balance;
            if (balance < 0) {
                throw new IllegalStateException(owner + " starts in debt");
            }
        }
    }

    static class Savings extends Account {
        final int rate;

        Savings(String owner, int balance, int rate) {
            super(owner, balance);
            this.rate = checkedRate(rate);
        }

        static int checkedRate(int rate) {
            if (rate > 10) {
                throw new IllegalArgumentException("rate " + rate + " too high");
            }
            return rate;
        }
    }

    static class Share {
        final int each;

        Share(int total, int people) {
            each = total / people;
        }
    }

    static Range parse(String text) {
        int dash = text.indexOf('-');
        if (dash < 0) {
            return new Range(Integer.parseInt(text));
        }
        return new Range(Integer.parseInt(text.substring(0, dash)), Integer.parseInt(text.substring(dash + 1)));
    }

    public static void main(String[] args) {
        String[] inputs = {"3-7", "9-2", "5", "-1", "x"};
        for (String input : inputs) {
            try {
                Range r = parse(input);
                System.out.println(input + " has " + r.size());
            } catch (IllegalArgumentException e) {
                System.out.println(input + ": " + e.getMessage());
            }
        }
        System.out.println("built " + built);
        int[][] accounts = {{100, 2}, {-5, 1}, {50, 20}};
        int opened = 0;
        for (int[] a : accounts) {
            try {
                Savings s = new Savings("owner" + a[0], a[0], a[1]);
                opened++;
                System.out.println(s.owner + " at " + s.rate + "% with " + s.balance);
            } catch (IllegalStateException | IllegalArgumentException e) {
                System.out.println("refused: " + e.getMessage());
            } finally {
                System.out.println("checked " + a[0]);
            }
        }
        System.out.println("opened " + opened);
        for (int people = 2; people >= 0; people--) {
            try {
                System.out.println(new Share(10, people).each);
            } catch (ArithmeticException e) {
                System.out.println("nobody to share with");
            }
        }
        Range last = new Range(1, 0);
        System.out.println(last.size());
    }
}
