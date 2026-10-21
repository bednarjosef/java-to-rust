// Classes of the program that extend `Number`, as the JDK's `AtomicInteger` and `BigInteger` do:
// their `intValue` to `doubleValue`, declared or inherited from a superclass, run through a
// `Number` beside the boxes, and `Number`'s own `byteValue` and `shortValue` narrow their
// `intValue`.
public class NumberSubclasses {
    static class Fraction extends Number {
        final long num;
        final long den;

        Fraction(long num, long den) {
            this.num = num;
            this.den = den;
        }

        @Override
        public int intValue() {
            return (int) longValue();
        }

        @Override
        public long longValue() {
            return num / den;
        }

        @Override
        public float floatValue() {
            return (float) doubleValue();
        }

        @Override
        public double doubleValue() {
            return (double) num / den;
        }

        int narrowed() {
            return byteValue() + shortValue();
        }

        @Override
        public String toString() {
            return num + "/" + den;
        }
    }

    // A counter whose number methods come from an abstract superclass that does not declare them.
    abstract static class Cell extends Number {
        long value;

        void add(long x) {
            value += x;
        }

        @Override
        public String toString() {
            return "cell " + value;
        }
    }

    static class Counter extends Cell {
        @Override
        public int intValue() {
            return (int) value;
        }

        @Override
        public long longValue() {
            return value;
        }

        @Override
        public float floatValue() {
            return value;
        }

        @Override
        public double doubleValue() {
            return value;
        }
    }

    static <T extends Number> double sum(T[] numbers) {
        double total = 0;
        for (Number n : numbers) {
            total += n.doubleValue();
        }
        return total;
    }

    public static void main(String[] args) {
        Fraction f = new Fraction(1001, 2);
        Counter c = new Counter();
        c.add(70000);
        c.add(-3);
        Number[] numbers = {3, 2.5, f, c, 40000L, (short) -7, 1e10f};
        for (Number n : numbers) {
            System.out.println(n + " " + n.intValue() + " " + n.longValue() + " " + n.byteValue() + " "
                    + n.shortValue() + " " + n.floatValue() + " " + n.doubleValue());
        }
        System.out.println(sum(numbers));
        System.out.println(f.byteValue() + " " + f.shortValue() + " " + f.narrowed() + " " + c.shortValue());
        Object o = c;
        System.out.println(o instanceof Number);
        Number n = f;
        System.out.println(n == f);
    }
}
