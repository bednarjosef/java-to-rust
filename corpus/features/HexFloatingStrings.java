public class HexFloatingStrings {
    static void showDouble(double d) {
        String text = Double.toHexString(d);
        System.out.println(text + " " + Double.parseDouble(text) + " " + (Double.parseDouble(text) == d));
    }

    static void showFloat(float f) {
        String text = Float.toHexString(f);
        System.out.println(text + " " + Float.parseFloat(text) + " " + (Float.parseFloat(text) == f));
    }

    public static void main(String[] args) {
        double[] doubles = {
            0.0, -0.0, Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, 1.0, -1.0,
            Double.MIN_VALUE, -Double.MIN_VALUE, Double.MIN_NORMAL, Double.MAX_VALUE, -Double.MAX_VALUE,
            Double.MIN_NORMAL - Double.MIN_VALUE, 0.1, 3.0, 1.5, -2.5, 0.5, 255.5, 1024.0, 1.0 / 3,
            1e300, 1e-300, Math.PI, 2 * Double.MIN_VALUE, 0x1.8p-1040,
        };
        for (double d : doubles) {
            System.out.println(Double.toHexString(d));
        }
        for (double d : doubles) {
            showDouble(d);
        }
        float[] floats = {
            0.0f, -0.0f, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, 1.0f, -1.0f,
            Float.MIN_VALUE, -Float.MIN_VALUE, Float.MIN_NORMAL, Float.MAX_VALUE, -Float.MAX_VALUE,
            Float.MIN_NORMAL - Float.MIN_VALUE, 0.1f, 3.0f, 1.5f, -2.5f, 0.5f, 255.5f, 65504f,
            1e-40f, 1e10f, 0.33333334f, (float) Math.PI, 2 * Float.MIN_VALUE,
        };
        for (float f : floats) {
            System.out.println(Float.toHexString(f));
        }
        for (float f : floats) {
            showFloat(f);
        }
        System.out.println(Double.toHexString(Float.MIN_VALUE) + " " + Float.toHexString(Float.MIN_VALUE));
        System.out.println(Double.toHexString(1) + " " + Float.toHexString(1) + " " + Float.toHexString('a'));
    }
}
