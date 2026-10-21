public class ThrowingInitializersInSubclasses {
    static int divisor = 0;

    static class Base {
        final int q = 12 / divisor;

        Base() {
            System.out.println("base " + q);
        }
    }

    static class Derived extends Base {
        final int r;

        Derived(int r) {
            this.r = r;
            System.out.println("derived " + r);
        }
    }

    static class Leaf extends Derived {
        Leaf() {
            super(7);
        }
    }

    public static void main(String[] args) {
        try {
            new Derived(1);
        } catch (ArithmeticException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            new Leaf();
        } catch (ArithmeticException e) {
            System.out.println("leaf caught " + e.getMessage());
        }
        divisor = 4;
        Derived d = new Derived(2);
        System.out.println("sum " + (d.r + d.q));
        System.out.println("leaf " + new Leaf().q);
        Base b = new Base();
        divisor = 0;
        System.out.println("end " + b.q + " " + new Derived(3).r);
    }
}
