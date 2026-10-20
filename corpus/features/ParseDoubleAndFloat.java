public class ParseDoubleAndFloat {
    static void show(String text) {
        System.out.println(Double.parseDouble(text) + " " + Float.parseFloat(text));
    }

    public static void main(String[] args) {
        String[] inputs = {
            "1.5", "  -2.25  ", "+7", ".5", "5.", "1e3", "1E-3", "2.5d", "3.5F", "0x1.8p3",
            "0X1p-2", "-0x1.fffffffffffffp1023", "Infinity", "-Infinity", "NaN", "\t 4.0\n",
            "1.4e-45", "3.4028235e38", "1e400", "16777217", "0.1", "-0.0", "1e-400",
            "0x1.000002p0f", "0x.8p1", "0x1.8P+3d", "3.4028235677973366e38",
        };
        for (String text : inputs) {
            show(text);
        }
        System.out.println(Float.valueOf("2.5") + " " + Double.valueOf("0x1p-1074"));
        System.out.println(Float.parseFloat("1.00000005") + " " + Double.parseDouble("1.00000005"));
        System.out.println(Double.parseDouble("1.5x"));
    }
}
