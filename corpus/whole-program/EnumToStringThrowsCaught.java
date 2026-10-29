import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * An enum whose {@code toString} throws, where a {@code catch} sees the exception: the text that
 * a concatenation, a print, {@code String.valueOf} or a format makes calls it, and what it throws
 * is caught, also where only one constant's body overrides it. The text that a collection makes
 * of its elements ends the program with it.
 */
public class EnumToStringThrowsCaught {
    enum Cache {
        UNIFIED, DATA, TRACE, L4;

        @Override
        public String toString() {
            if (this == TRACE) {
                throw new IllegalStateException("no text for " + name());
            }
            // A name of two letters has no third: StringIndexOutOfBoundsException.
            return name().substring(0, 1) + name().substring(1, 3).toLowerCase(Locale.ROOT);
        }
    }

    // Only one constant's own body throws.
    enum Mode {
        QUIET,
        LOUD {
            @Override
            public String toString() {
                throw new UnsupportedOperationException("too loud");
            }
        }
    }

    record Level(int number, Cache cache) {
        String describe() {
            return "L" + number + " " + cache;
        }
    }

    static String describe(Cache c) {
        return "cache " + c;
    }

    public static void main(String[] args) {
        List<String> seen = new ArrayList<>();
        int n = 0;
        for (Cache c : Cache.values()) {
            n++;
            try {
                System.out.println(describe(c));
            } catch (IllegalStateException e) {
                System.out.println("caught " + e.getMessage());
            } catch (RuntimeException e) {
                System.out.println("caught " + e.getClass().getName());
            }
            try {
                System.out.println(c);
            } catch (RuntimeException e) {
                System.out.println("println " + e.getClass().getName());
            }
            try {
                seen.add(String.valueOf(c));
            } catch (RuntimeException e) {
                seen.add("none");
            }
            try {
                System.out.println(String.format("%s!", c));
            } catch (RuntimeException e) {
                System.out.println("format " + e.getClass().getSimpleName());
            }
            String s = "s";
            try {
                s += c;
            } catch (RuntimeException e) {
                s += "?";
            }
            System.out.println(s);
            Cache maybe = n % 2 == 0 ? null : c;
            try {
                System.out.println("maybe " + maybe);
            } catch (RuntimeException e) {
                System.out.println("maybe " + e.getClass().getSimpleName());
            }
            try {
                System.out.println(new Level(n, c).describe());
            } catch (Exception e) {
                System.out.println("level " + n + ": " + e.getClass().getSimpleName());
            }
        }
        for (Mode m : Mode.values()) {
            try {
                System.out.println("mode " + m);
            } catch (UnsupportedOperationException e) {
                System.out.println("mode caught " + e.getMessage());
            }
        }
        System.out.println(seen);
        System.out.println(List.of(Cache.UNIFIED, Cache.DATA));
        // A collection's text runs the toString where no catch sees it.
        System.out.println(List.of(Cache.DATA, Cache.TRACE));
    }
}
