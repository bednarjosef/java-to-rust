// String.format, formatted and printf: integers in decimal, hex and octal with widths, flags and
// grouping; floating point with %f, %e and %g, rounded half up from the shortest decimal as Java
// does (0.125 is 0.13, 0.15 is 0.2); strings, chars, booleans, objects, enums and nulls; argument
// indexes and %n. The format of the last line wants an argument that is missing, which throws
// MissingFormatArgumentException.
public class StringFormat {
    enum Level { LOW, HIGH }

    static class Money {
        long cents;

        Money(long cents) {
            this.cents = cents;
        }

        public String toString() {
            return String.format("$%d.%02d", cents / 100, cents % 100);
        }
    }

    static String row(String name, int count, double share) {
        return String.format("%-8s|%5d|%7.2f%%", name, count, share * 100);
    }

    public static void main(String[] args) {
        System.out.println(String.format("%d %d %d %d", 0, -7, Integer.MAX_VALUE, Long.MIN_VALUE));
        System.out.println(String.format("[%5d] [%-5d] [%05d] [%+d] [% d] [%(d]", 42, 42, 42, 42, 42, -42));
        System.out.println(String.format("%,d %,d %,d %,015d", 1234567, -9876543210L, 999, 1234567));
        System.out.println(String.format("%x %X %o %x %08X %#x %#o", 255, 255, 8, -1, 48879, 255, 8));
        System.out.println(String.format("%x %x %o", -1L, (byte) -1, (short) -2));
        System.out.println(String.format("%f %.2f %.0f %.3f %10.4f|%-10.1f|", 3.14159, 2.675, 2.5, -0.0005, Math.PI, 1.25));
        System.out.println(String.format("%.1f %.1f %.2f %.2f %.1f", 0.15, 0.25, 0.125, 1.005, 0.05));
        System.out.println(String.format("%e %.2e %E %.0e", 12345.678, 0.000123, 1e100, 5.5));
        System.out.println(String.format("%g %g %g %.3g %g", 0.0001, 123456.0, 1234567.0, 99.95, 0.0));
        System.out.println(String.format("%,.2f %+.1f %(.2f %08.2f", 1234567.891, 3.0, -2.5, -3.14159));
        System.out.println(String.format("%f %f %.2f %e", Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, 1.5f));
        System.out.println(String.format("%s %S %10s|%-10s|%.3s", "text", "text", "right", "left", "truncate"));
        System.out.println(String.format("%c %c %C %b %b %B %h", 'a', 66, 'z', true, "x", false, "hello"));
        String missing = null;
        Integer nothing = null;
        System.out.println(String.format("%s %b %s %d", missing, missing, nothing, 5));
        System.out.println(String.format("%s %s %s %s", Level.HIGH, Level.valueOf("LOW"), new Money(12345), new Money(7)));
        System.out.println(String.format("%2$s %1$s %2$s %<s %s", "a", "b"));
        System.out.println(String.format("100%% done%nnext line"));
        System.out.println(String.format("no arguments") + String.format("", 1, 2));
        System.out.println(row("apples", 12, 0.4567) + "\n" + row("kiwis", 3, 0.05));
        System.out.println("%d items at %.2f".formatted(3, 9.5));
        System.out.printf("%s has %d chars%n", "printf", "printf".length());
        System.out.format("%5.1f|%n", 2.25);
        System.out.printf("plain%n");
        long big = 1L << 40;
        char letter = 'q';
        float f = 0.1f;
        System.out.println(String.format("%d %c %s %.10f %s", big, letter, f, f, 1.0f / 3));
        System.out.println(String.format("%s %d", "only one"));
    }
}
