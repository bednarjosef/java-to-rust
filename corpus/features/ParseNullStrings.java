public class ParseNullStrings {
    public static void main(String[] args) {
        try {
            Integer.parseInt(null);
        } catch (NumberFormatException e) {
            System.out.println("int " + e.getMessage());
        }
        try {
            Long.parseLong(null);
        } catch (NumberFormatException e) {
            System.out.println("long " + e.getMessage());
        }
        try {
            Short.parseShort(null);
        } catch (NumberFormatException e) {
            System.out.println("short " + e.getMessage());
        }
        try {
            Byte.parseByte(null);
        } catch (NumberFormatException e) {
            System.out.println("byte " + e.getMessage());
        }
        try {
            Integer.valueOf(null);
        } catch (NumberFormatException e) {
            System.out.println("valueOf " + e.getMessage());
        }
        try {
            Long.valueOf(null, 16);
        } catch (NumberFormatException e) {
            System.out.println("radix " + e.getMessage());
        }
        try {
            Short.parseShort("32768");
        } catch (NumberFormatException e) {
            System.out.println("short range " + e.getMessage());
        }
        try {
            Byte.parseByte("ff", 16);
        } catch (NumberFormatException e) {
            System.out.println("byte radix " + e.getMessage());
        }
        try {
            Integer.parseInt("3000000000", 10);
        } catch (NumberFormatException e) {
            System.out.println("int big " + e.getMessage());
        }
        System.out.println(Short.parseShort("-32768") + Byte.parseByte("7f", 16));
        System.out.println(Integer.parseInt(null == null ? "41" : "0") + 1);
        Integer.parseInt(null);
    }
}
