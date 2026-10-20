public class Floats {
    static float average(float a, float b) {
        return (a + b) / 2;
    }

    static void show(float f) {
        System.out.println("float " + f);
    }

    static void show(double d) {
        System.out.println("double " + d);
    }

    static float sum(int n) {
        float total = 0;
        for (int i = 1; i <= n; i++) {
            total += 1.0f / i;
        }
        return total;
    }

    static int halvings(float f) {
        int count = 0;
        while (!(f < 1)) {
            f /= 2;
            count++;
            if (count > 200) {
                break;
            }
        }
        return count;
    }

    public static void main(String[] args) {
        float tenth = 0.1f;
        System.out.println(tenth);
        System.out.println((double) tenth);
        System.out.println(tenth + 0.2f);
        System.out.println(tenth + 0.2);
        System.out.println(1.0f / 3);
        System.out.println(2f / 3f + " " + 1e10f + " " + 1.5e-5f + " " + 123456.78f + " " + 1e7f + " " + 9999999f);
        System.out.println(100.0f + " " + 0.001f + " " + 0.0001f + " " + -2.5f + " " + 3f);

        // Special values.
        float zero = 0.0f;
        float negZero = -0.0f;
        float nan = zero / zero;
        float inf = 1 / zero;
        System.out.println(nan + " " + inf + " " + -inf + " " + negZero + " " + (zero == negZero) + " " + (1 / negZero));
        System.out.println((nan == nan) + " " + (nan != nan) + " " + (nan < 1) + " " + (nan > 1) + " " + !(nan >= 1));
        System.out.println(Float.MAX_VALUE + " " + Float.MIN_VALUE + " " + -Float.MAX_VALUE + " " + Float.NaN);
        System.out.println(Float.POSITIVE_INFINITY + " " + Float.NEGATIVE_INFINITY + " " + (Float.MAX_VALUE * 2));
        System.out.println(Float.MIN_VALUE / 2 + " " + (double) Float.MIN_VALUE + " " + (float) 1e-50 + " " + (float) -1e300);
        System.out.println(Double.MAX_VALUE + " " + Double.MIN_VALUE + " " + Double.NaN + " " + Double.NEGATIVE_INFINITY);

        // Conversions.
        System.out.println((int) 3.9f + " " + (int) -3.9f + " " + (int) nan + " " + (int) inf + " " + (int) -inf);
        System.out.println((long) 1e19f + " " + (long) -1e19f + " " + (long) nan + " " + (int) 2.5e9f);
        System.out.println((float) 16777217 + " " + (float) Long.MAX_VALUE + " " + (float) 0.1 + " " + (float) 1e40);
        int big = 123456789;
        long huge = 9007199254740993L;
        float fromInt = big;
        float fromLong = huge;
        double fromFloat = fromInt;
        System.out.println(fromInt + " " + fromLong + " " + fromFloat + " " + (int) fromInt);
        char c = 'A';
        float fromChar = c;
        System.out.println(fromChar + " " + (char) (fromChar + 1.7f) + " " + (byte) 300.5f + " " + (short) -40000.5f);

        // Arithmetic is float arithmetic, unless a double takes part.
        float f = 1.1f;
        f *= 3;
        System.out.println(f);
        f += 0.1;
        System.out.println(f);
        f -= 1;
        System.out.println(f);
        f /= 0;
        System.out.println(f);
        f = 7.5f;
        f %= 2;
        System.out.println(f + " " + (-7.5f % 2) + " " + (7.5f % -2) + " " + (5f % 0));
        f++;
        ++f;
        f--;
        System.out.println(f + " " + -f + " " + +f);
        float x = 16777216f;
        x++;
        System.out.println(x + " " + (x + 1 == x) + " " + ((double) x + 1 == x));
        System.out.println(average(1, 2) + " " + average(Float.MAX_VALUE, Float.MAX_VALUE) + " " + sum(100));
        System.out.println(halvings(1000f) + " " + halvings(nan) + " " + halvings(0.5f));
        int i = 3;
        long l = 4;
        System.out.println(i / 2f + " " + l * 0.5f + " " + (i + 0.5f) + " " + (i + 0.5));

        // Math on floats.
        System.out.println(Math.abs(-2.5f) + " " + Math.abs(negZero) + " " + Math.max(1.5f, 2) + " " + Math.min(-1f, 3f));
        System.out.println(Math.max(nan, 1f) + " " + Math.min(1f, nan) + " " + Math.max(negZero, zero) + " " + Math.min(zero, negZero));
        System.out.println(Math.sqrt(2f) + " " + (float) Math.sqrt(2f));
        show(1.5f);
        show(1.5);
        show(f);
        show(f * 2.0);

        // Comparisons.
        float a = 0.5f;
        System.out.println((a < 1) + " " + (a == 0.5) + " " + (a == 0.5f) + " " + (0.1f == 0.1) + " " + (a > big) + " " + (fromInt == big));
        float g = 0;
        int steps = 0;
        while (g < 1) {
            g += 0.1f;
            steps++;
        }
        System.out.println(steps + " " + g);
    }
}
