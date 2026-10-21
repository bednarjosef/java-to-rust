public class ParseRangeAndRadix {
    static void show(String label, Runnable action) {
        try {
            action.run();
        } catch (NumberFormatException e) {
            System.out.println(label + " NFE " + e.getMessage());
        } catch (IndexOutOfBoundsException e) {
            System.out.println(label + " IOOBE " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        String s = "x-12ff7";
        System.out.println(Integer.parseInt(s, 1, 6, 16));
        System.out.println(Long.parseLong("x9223372036854775807", 1, 20, 10));
        System.out.println(Integer.parseUnsignedInt("x4294967295", 1, 11, 10));
        System.out.println(Long.parseUnsignedLong("x18446744073709551615", 1, 21, 10));
        show("int-empty", () -> Integer.parseInt(s, 2, 2, 16));
        show("int-minus", () -> Integer.parseInt("x-", 1, 2, 10));
        show("int-bad", () -> Integer.parseInt("x1g", 0, 3, 16));
        show("int-over", () -> Integer.parseInt("2147483648", 0, 10, 10));
        show("int-over-sub", () -> Integer.parseInt("x2147483648y", 1, 11, 10));
        show("int-radix", () -> Integer.parseInt(s, 0, 3, 1));
        show("int-radix-big", () -> Integer.parseInt(s, 0, 3, 37));
        show("int-begin", () -> Integer.parseInt(s, 4, 2, 16));
        show("int-end", () -> Integer.parseInt(s, 0, 9, 16));
        show("int-neg", () -> Integer.parseInt(s, -1, 2, 16));
        show("long-over", () -> Long.parseLong("9223372036854775808", 0, 19, 10));
        show("long-plus", () -> Long.parseLong("+", 0, 1, 10));
        show("long-empty", () -> Long.parseLong("abc", 3, 3, 10));
        show("uint-minus", () -> Integer.parseUnsignedInt("x-1", 1, 3, 10));
        show("uint-over", () -> Integer.parseUnsignedInt("x4294967296", 1, 11, 10));
        show("uint-over-sub", () -> Integer.parseUnsignedInt("ab4294967296", 2, 12, 10));
        show("uint-bad", () -> Integer.parseUnsignedInt("x1z", 1, 3, 16));
        show("ulong-over", () -> Long.parseUnsignedLong("x18446744073709551616", 1, 21, 10));
        show("ulong-minus", () -> Long.parseUnsignedLong("-5", 0, 2, 10));
        show("ulong-empty", () -> Long.parseUnsignedLong("abc", 1, 1, 10));
        show("ulong-bad", () -> Long.parseUnsignedLong("q1z", 1, 3, 36));
        show("short-range", () -> System.out.println(Short.parseShort("7fff", 16)));
        show("short-over", () -> System.out.println(Short.parseShort("8000", 16)));
        show("short-big", () -> System.out.println(Short.parseShort("99999999999")));
        show("short-value", () -> System.out.println(Short.valueOf("8000", 16)));
        show("byte-over", () -> System.out.println(Byte.parseByte("80", 10)));
        show("byte-under", () -> System.out.println(Byte.parseByte("-81", 10)));
        show("byte-radix", () -> System.out.println(Byte.parseByte("-80", 16)));
        show("byte-bad", () -> System.out.println(Byte.parseByte("zz", 10)));
        show("byte-valueof", () -> System.out.println(Byte.valueOf("7f", 16)));
        show("short-bad-radix", () -> System.out.println(Short.parseShort("7", 99)));
        System.out.println(Integer.valueOf("ff", 16));
        System.out.println(Long.valueOf("ff", 16));
        System.out.println(Integer.parseUnsignedInt("ff", 16));
        System.out.println(Long.parseUnsignedLong("ff", 16));
        System.out.println(Integer.parseUnsignedInt("+7", 0, 2, 10));
        System.out.println(Short.parseShort("+7", 10));
    }
}
