public class ThrowingConstructorsCaught {
    static int divisor = 3;

    static class Thing {
        final int value;

        Thing(int v) {
            if (v < 0) {
                throw new IllegalArgumentException("negative: " + v);
            }
            value = v;
        }
    }

    static class Base {
        final int seen;

        Base(int v) {
            if (v > 10) {
                throw new IllegalStateException("too big: " + v);
            }
            seen = v;
        }
    }

    static class Derived extends Base {
        Derived(int v) {
            super(v * 2);
        }
    }

    static class Chain {
        final int n;

        Chain() {
            this(-5);
        }

        Chain(int a) {
            n = new Thing(a).value;
        }
    }

    static class Ratio {
        final int q = 12 / divisor;
    }

    static int check(int v) {
        if (v == 7) {
            throw new IllegalStateException("seven");
        }
        return v;
    }

    static class Checked {
        final int c = check(divisor + 4);

        Checked(String name) {
            System.out.println("built " + name + " " + c);
        }
    }

    public static void main(String[] args) {
        try {
            new Thing(-1);
        } catch (IllegalArgumentException e) {
            System.out.println("thing: " + e.getMessage());
        }
        System.out.println("thing ok: " + new Thing(4).value);
        try {
            new Derived(6);
        } catch (IllegalStateException e) {
            System.out.println("derived: " + e.getMessage());
        }
        System.out.println("derived ok: " + new Derived(2).seen);
        try {
            new Chain();
        } catch (IllegalArgumentException e) {
            System.out.println("chain: " + e.getMessage());
        }
        System.out.println("chain ok: " + new Chain(9).n);
        divisor = 0;
        try {
            new Ratio();
        } catch (ArithmeticException e) {
            System.out.println("ratio: " + e.getMessage());
        }
        divisor = 2;
        System.out.println("ratio ok: " + new Ratio().q);
        divisor = 3;
        try {
            new Checked("first");
        } catch (IllegalStateException e) {
            System.out.println("checked: " + e.getMessage());
        }
        divisor = 1;
        new Checked("second");
        divisor = 0;
        System.out.println("last: " + new Ratio().q);
    }
}
