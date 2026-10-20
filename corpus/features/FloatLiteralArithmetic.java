// Arithmetic of float literals is done in float, even where the result is widened to double or
// compared, as Java does.
public class FloatLiteralArithmetic {
    public static void main(String[] args) {
        System.out.println((double) (2.0f / 3));
        System.out.println(2.0f / 3);
        double third = 1.0f / 3.0f;
        System.out.println(third);
        System.out.println((double) (0.1f + 0.2f));
        System.out.println((long) (16777216.0f + 1.0f));
        System.out.println(0.1f + 0.2f == 0.3f);
        System.out.println(1.0e-45f / 2 < 1.0e-45f);
        System.out.println((double) (2.0f / 3) * 3);
        System.out.println((int) (3.4e38f * 10.0f));
    }
}
