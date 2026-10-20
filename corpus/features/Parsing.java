public class Parsing {
    static Integer parsed(String text) {
        return Integer.valueOf(text);
    }

    public static void main(String[] args) {
        Integer a = Integer.parseInt("42");
        Long b = Long.parseLong("-9000000000");
        Double c = Double.parseDouble("1d");
        Double d = Double.parseDouble(" 2.5 ");
        Double e = Double.valueOf("0x1p3");
        Boolean f = Boolean.parseBoolean("TRUE");
        Boolean g = Boolean.valueOf("yes");
        Integer h = parsed("-17");
        System.out.println(a + " " + b + " " + c + " " + d + " " + e + " " + f + " " + g + " " + h);
        System.out.println(Integer.parseInt("+5") + Integer.parseInt("007"));
        System.out.println(Boolean.parseBoolean(null) + " " + Character.getNumericValue('z') + " " + Character.getNumericValue('7'));
        System.out.println(Integer.toString(255) + " " + Long.toString(-3L) + " " + Double.toString(0.5));
        System.out.println(Integer.compare(3, 9) + " " + Long.compare(9L, 3L) + " " + Double.compare(1.0, 1.0));
        System.out.println(Double.valueOf("NaN").equals(Double.NaN) + " " + Double.valueOf(0.0).equals(-0.0));
        System.out.println(Integer.parseInt("12x"));
    }
}
