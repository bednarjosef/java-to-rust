import java.util.Arrays;

// Constant regular expressions that the runtime does not support (properties, flags, lookbehind,
// named groups) run on the JDK's own `Pattern`, as computed ones do; those it supports (here
// boundaries, lookahead and possessive quantifiers) stay the runtime's.
public class UnsupportedConstantPatterns {
    static final String ZEROS = "((?:(?:^|:)0+\\b){2,8}):?(?!\\S*\\b\\1:0+\\b)(\\S*)";

    public static void main(String[] args) {
        // An IPv6 address with its longest run of zero groups shortened.
        for (String address : new String[] {"fe80:0:0:0:0:0:0:1", "1:0:0:2:0:0:0:3", "0:0:0:0:0:0:0:0"}) {
            System.out.println(address.replaceAll(ZEROS, "::$2"));
        }
        System.out.println("café naïve plain".replaceAll("\\P{InBasic_Latin}", "?"));
        System.out.println(Arrays.toString("one twoéthree".split("\\b")));
        System.out.println("Hello hello HELLO".replaceAll("(?i)hello", "hi"));
        System.out.println("aaa".matches("a++a") + " " + "aaa".matches("a+a"));
        System.out.println("2026-10-24".replaceFirst("(?<y>\\d+)-(?<m>\\d+)-(?<d>\\d+)", "${d}.${m}.${y}"));
        System.out.println("x1y22z".replaceAll("\\d+", "#") + " " + "price: 10".replaceAll("(?<=: )\\d+", "N"));
        System.out.println(Arrays.toString("a1b2c3".split("(?=\\d)")));
        try {
            System.out.println("text".replaceAll("\\p{Bogus}", "x"));
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getClass().getName());
        }
    }
}
