// An enum constructor that throws: the enum's initialization fails with an
// ExceptionInInitializerError at its first use, which ends the program.
public class EnumConstructorThrows {
    enum Ratio {
        HALF(1, 2),
        BROKEN(1, 0);

        final int percent;

        Ratio(int numerator, int denominator) {
            if (denominator == 0) {
                throw new IllegalArgumentException("zero denominator");
            }
            percent = numerator * 100 / denominator;
        }
    }

    public static void main(String[] args) {
        System.out.println("before");
        System.out.println(Ratio.HALF.percent);
        System.out.println("after");
    }
}
