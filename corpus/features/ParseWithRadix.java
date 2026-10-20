public class ParseWithRadix {
    public static void main(String[] args) {
        System.out.println(Integer.parseInt("-ff", 16));
        System.out.println(Integer.parseInt("+777", 8));
        System.out.println(Integer.parseInt("zz", 36));
        System.out.println(Integer.parseInt("Zz", 36) + " " + Integer.parseInt("-0", 36));
        System.out.println(Integer.parseInt("1010", 2));
        System.out.println(Integer.parseInt("-80000000", 16));
        System.out.println(Integer.parseInt("2147483647", 10));
        System.out.println(Long.parseLong("7fffffffffffffff", 16));
        System.out.println(Long.parseLong("-9223372036854775808", 10));
        System.out.println(Integer.valueOf("123", 10) + Integer.valueOf("-1", 2));
        // ARABIC-INDIC DIGITS ONE TWO, FULLWIDTH DIGIT THREE, and fullwidth Latin letters.
        System.out.println(Integer.parseInt("١٢", 10));
        System.out.println(Integer.parseInt("３", 10) + " " + Integer.parseInt("Ａａ", 16));
        System.out.println(Long.valueOf("०३", 10));
        String missing = args.length > 5 ? "1" : null;
        System.out.println(Integer.parseInt(missing, 16));
    }
}
