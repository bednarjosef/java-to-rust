public class MathPrinting {
    public static void main(String[] args) {
        int a = -17;
        long b = -9000000000L;
        System.out.println(Math.abs(a) + " " + Math.abs(b));
        System.out.println(Math.max(a, 3) + " " + Math.min(b, 0L));
        System.out.println(Math.max(1.5f, 2) + " " + Math.min(-3, 2.5));
        System.out.println(Math.pow(2, 10));
        System.out.println(Math.pow(2, -1074));
        System.out.println(Math.pow(10, 15));
        System.out.println(Math.pow(-8, 1.0 / 3));
        System.out.println(Math.sqrt(144));
        System.out.println(Math.cbrt(-27));
        System.out.println(Math.floor(-2.5) + " " + Math.ceil(-2.5) + " " + Math.rint(2.5));
        System.out.println(Math.round(-0.5) + " " + Math.round(0.49999999999999994));
        System.out.println(Math.round(Float.NaN) + " " + Math.round(1e30));
        System.out.println(Math.floorMod(-7L, 3) + " " + Math.floorDiv(-7L, 2L));
        System.out.println(Math.toRadians(180));
        System.out.println(Math.toDegrees(Math.PI / 2));
        System.out.println(Math.sin(0) + " " + Math.cos(0));
        System.out.println(Math.exp(1) + " " + Math.log(Math.E));
        System.out.println(Math.log10(1000));
        System.out.println(Math.atan2(1, 1) * 4);
        System.out.println(Math.hypot(1e200, 1e200));
        System.out.println(Math.abs(-0.0f));
        System.out.println(Math.signum(0.0));
        System.out.println((int) Math.signum(-5L));
        System.out.println(Math.max(Integer.MIN_VALUE, Integer.MIN_VALUE + 1));
    }
}
