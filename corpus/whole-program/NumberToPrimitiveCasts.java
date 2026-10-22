import java.math.BigInteger;

// A cast of a reference that is not a box to a primitive: `(long) number` checks that the
// `Number` is a `Long` and unboxes it, as `CompactNumberFormat.computeDivisor` does, and so do
// casts from `Comparable` and `Object`.
public class NumberToPrimitiveCasts {
    static Number power(int i) {
        if (i < 3) {
            return (long) Math.pow(10, i);
        }
        return BigInteger.TEN.pow(i);
    }

    static Number divisor(Number value, int digits) {
        if (value instanceof BigInteger big) {
            return big.divide(BigInteger.TEN.pow(digits));
        }
        long longValue = (long) value;
        if (longValue < (long) Math.pow(10, digits)) {
            throw new IllegalArgumentException("too many digits: " + digits);
        }
        return longValue / (long) Math.pow(10, digits);
    }

    public static void main(String[] args) {
        for (int i = 0; i < 5; i++) {
            try {
                System.out.println(i + " " + divisor(power(i), 1));
            } catch (IllegalArgumentException e) {
                System.out.println(i + " " + e.getMessage());
            }
        }
        Comparable<Integer> comparable = 7;
        Object object = 40L;
        int fromComparable = (int) comparable;
        long fromObject = (long) object;
        System.out.println(fromComparable + " " + fromObject);
        Number wrong = Integer.valueOf(3);
        try {
            long notLong = (long) wrong;
            System.out.println(notLong);
        } catch (ClassCastException e) {
            System.out.println("not a Long");
        }
    }
}
