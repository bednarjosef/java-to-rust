// `finally` runs however its `try` ends: normally, by an exception that goes on to a caller or
// ends the program, by a `return` (whose value is taken before the `finally` runs), and by a
// `break` or `continue` out of the `try`, also through nested `try`s.
public class Finally {
    static class InsufficientFunds extends RuntimeException {
        final int missing;

        InsufficientFunds(int missing) {
            super("missing " + missing);
            this.missing = missing;
        }
    }

    static int balance = 100;
    static int operations = 0;

    static void withdraw(int amount) {
        try {
            System.out.println("withdraw " + amount);
            if (amount > balance) {
                throw new InsufficientFunds(amount - balance);
            }
            balance -= amount;
        } finally {
            operations++;
            System.out.println("operations " + operations);
        }
    }

    static int find(int[] values, int target) {
        try {
            for (int i = 0; i < values.length; i++) {
                if (values[i] == target) {
                    return i;
                }
            }
            return -1;
        } finally {
            System.out.println("searched for " + target);
        }
    }

    static int counter = 0;

    static int next() {
        try {
            counter++;
            return counter;
        } finally {
            counter += 10;
        }
    }

    static int sumUntil(int[] values, int stop) {
        int sum = 0;
        for (int v : values) {
            try {
                if (v == stop) {
                    break;
                }
                if (v < 0) {
                    continue;
                }
                sum += v;
            } finally {
                System.out.println("saw " + v);
            }
        }
        return sum;
    }

    static String describe(int n) {
        try {
            try {
                if (n < 0) {
                    throw new IllegalArgumentException("negative");
                }
                return "n=" + n;
            } finally {
                System.out.println("inner " + n);
            }
        } catch (IllegalArgumentException e) {
            return "bad: " + e.getMessage();
        } finally {
            System.out.println("outer " + n);
        }
    }

    static void visit(int n) {
        for (int i = 0; i < n; i++) {
            try {
                try {
                    if (i == 1) {
                        continue;
                    }
                    if (i == 3) {
                        return;
                    }
                    System.out.println("visit " + i);
                } finally {
                    System.out.println("inner " + i);
                }
            } finally {
                System.out.println("outer " + i);
            }
        }
        System.out.println("visited all");
    }

    static String label(String name) {
        String upper = name.toUpperCase();
        try {
            return upper;
        } finally {
            System.out.println("label of " + upper);
        }
    }

    static class Point {
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        @Override
        public String toString() {
            try {
                if (x == 0 && y == 0) {
                    return "origin";
                }
                return "(" + x + ", " + y + ")";
            } finally {
                System.out.println("described");
            }
        }
    }

    static int classify(int n) {
        int kind = n * 2;
        try {
            switch (n) {
                case 1:
                    kind += 10;
                    break;
                case 2:
                    kind += 20;
                    break;
                default:
                    kind -= 1;
            }
        } finally {
            System.out.println("classified " + n);
        }
        return kind;
    }

    static void deep(int n) {
        try {
            if (n > 2) {
                throw new IllegalStateException("too deep: " + n);
            }
            deep(n + 1);
        } finally {
            System.out.println("leaving " + n);
        }
    }

    public static void main(String[] args) {
        withdraw(30);
        try {
            withdraw(500);
        } catch (InsufficientFunds e) {
            System.out.println("caught " + e.getMessage() + ", " + e.missing);
        }
        System.out.println("balance " + balance);
        System.out.println(find(new int[] {4, 5, 6}, 5));
        System.out.println(find(new int[] {4, 5, 6}, 9));
        System.out.println(next() + " " + next() + " " + counter);
        System.out.println(sumUntil(new int[] {1, -2, 3, 7, 4}, 7));
        System.out.println(describe(3));
        System.out.println(describe(-3));
        visit(5);
        System.out.println(label("box"));
        System.out.println(new Point(0, 0));
        System.out.println(new Point(2, 3));
        System.out.println(classify(1) + classify(2) + classify(5));
        grid:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                try {
                    if (j == 1) {
                        continue grid;
                    }
                    if (i == 2) {
                        break grid;
                    }
                    System.out.println(i + "," + j);
                } finally {
                    System.out.println("cell " + i + "," + j);
                }
            }
        }
        deep(0);
    }
}
