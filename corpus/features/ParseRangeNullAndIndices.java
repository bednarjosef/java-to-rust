public class ParseRangeNullAndIndices {
    public static void main(String[] args) {
        String missing = args.length > 100 ? "1" : null;
        try {
            Integer.parseInt(missing, 0, 1, 10);
        } catch (NullPointerException e) {
            System.out.println("null string: NullPointerException");
        }
        try {
            Long.parseLong(missing, 0, 1, 10);
        } catch (NullPointerException e) {
            System.out.println("long null: NullPointerException");
        }
        String digits = "12345";
        System.out.println(Integer.parseInt(digits, 1, 3, 10));
        System.out.println(Long.parseLong("ff", 0, 2, 16));
        try {
            Integer.parseInt(digits, 3, 1, 10);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("reversed: " + e.getMessage());
        }
        try {
            Integer.parseInt(digits, 2, 9, 10);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("past end: " + e.getMessage());
        }
        try {
            Integer.parseInt(digits, 2, 2, 10);
        } catch (NumberFormatException e) {
            System.out.println("empty: " + e.getMessage());
        }
        try {
            Integer.parseInt("12x", 0, 3, 10);
        } catch (NumberFormatException e) {
            System.out.println("bad digit: " + e.getMessage());
        }
        try {
            Integer.parseInt(digits, 0, 2, 40);
        } catch (NumberFormatException e) {
            System.out.println("radix: " + e.getMessage());
        }
        try {
            Integer.parseInt("3000000000", 0, 10, 10);
        } catch (NumberFormatException e) {
            System.out.println("big: " + e.getMessage());
        }
    }
}
