// `java.lang.Number` as a type: the numeric boxes as its values, its `byteValue()` to
// `doubleValue()` with Java's conversions, `T extends Number`, casts and `instanceof` from
// `Object`, printing and `equals` across boxes.
public class NumberValues {
    static double total(Number a, Number b) {
        return a.doubleValue() + b.doubleValue();
    }

    static <T extends Number> long widen(T t) {
        return t.longValue();
    }

    static class Holder<T extends Number> {
        final T value;

        Holder(T value) {
            this.value = value;
        }

        int asInt() {
            return value.intValue();
        }

        boolean larger(Holder<? extends Number> other) {
            return value.doubleValue() > other.value.doubleValue();
        }
    }

    static String describe(Object o) {
        if (o instanceof Number) {
            Number n = (Number) o;
            return "number " + n + " as int " + n.intValue();
        }
        return "not a number: " + o;
    }

    public static void main(String[] args) {
        Number[] nums = {1, 2.5, 3L, (short) 4, (byte) -5, 6.5f, 300, 1e20, Double.NaN, 1L << 40, -2.75};
        for (Number n : nums) {
            System.out.println(n + ": " + n.byteValue() + " " + n.shortValue() + " " + n.intValue() + " "
                    + n.longValue() + " " + n.floatValue() + " " + n.doubleValue());
        }
        System.out.println(total(1, 2.25));
        System.out.println(total(7L, (short) 3));
        System.out.println(widen(42) + widen(3.99) + widen((byte) 7));
        Holder<Integer> small = new Holder<>(12);
        Holder<Double> big = new Holder<>(99.9);
        System.out.println(small.asInt() + big.asInt());
        System.out.println(small.larger(big) + " " + big.larger(small));
        System.out.println(describe(17));
        System.out.println(describe(0.5f));
        System.out.println(describe("seventeen"));
        Number one = 1;
        Number oneLong = 1L;
        System.out.println(one.equals(oneLong) + " " + one.equals(1) + " " + one.hashCode());
    }
}
