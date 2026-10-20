// The methods of String: searching, slicing, case, trimming, replacing, comparing and
// converting, on ASCII text and on text with characters outside it (é, a surrogate pair).
// Indexes are UTF-16 code units, as in Java. The last line takes a substring out of range, which
// throws StringIndexOutOfBoundsException.
public class StringMethods {
    static String describe(String s) {
        return "[" + s + "] length " + s.length();
    }

    static int countOf(String text, String part) {
        int count = 0;
        int at = text.indexOf(part);
        while (at >= 0) {
            count++;
            at = text.indexOf(part, at + part.length());
        }
        return count;
    }

    public static void main(String[] args) {
        String s = "Hello, World";
        System.out.println(s.substring(7));
        System.out.println(s.substring(0, 5));
        System.out.println(s.substring(12).isEmpty());
        System.out.println(s.indexOf('o') + " " + s.indexOf('o', 5) + " " + s.indexOf('z'));
        System.out.println(s.lastIndexOf('o') + " " + s.lastIndexOf('o', 7) + " " + s.lastIndexOf('H', -1));
        System.out.println(s.indexOf("World") + " " + s.indexOf("") + " " + s.indexOf("", 40));
        System.out.println(s.lastIndexOf("l") + " " + s.lastIndexOf("") + " " + s.lastIndexOf("l", 3));
        System.out.println(s.contains("lo, W") + " " + s.contains("low"));
        System.out.println(s.startsWith("Hell") + " " + s.startsWith("World", 7) + " " + s.endsWith("ld"));
        System.out.println(s.startsWith("o", -1) + " " + s.startsWith("", 12) + " " + s.startsWith("", 13));
        System.out.println(countOf("abababa", "aba") + " " + countOf("banana", "an"));

        String padded = "  \t padded \n ";
        System.out.println(describe(padded.trim()));
        System.out.println(describe(padded.strip()));
        System.out.println(describe(padded.stripLeading()));
        System.out.println(describe(padded.stripTrailing()));
        System.out.println("   ".isBlank() + " " + "".isBlank() + " " + " x ".isBlank() + " " + "".isEmpty());

        System.out.println("MiXeD Case 123".toUpperCase() + " " + "MiXeD Case 123".toLowerCase());
        System.out.println("straße".toUpperCase() + " " + "ÀÉÎ".toLowerCase());

        System.out.println("banana".replace('a', 'o') + " " + "banana".replace("an", "AN"));
        System.out.println("aaa".replace("aa", "b") + " " + "abc".replace("", "-"));
        System.out.println("ab".repeat(3) + "|" + "x".repeat(0) + "|");
        System.out.println(String.join(", ", "a", "b", "c") + " " + String.join("-", new String[] {"x", "y"}));
        System.out.println("[" + String.join("+") + "]");

        System.out.println("apple".compareTo("banana") + " " + "b".compareTo("a") + " " + "abc".compareTo("ab"));
        System.out.println("abc".compareTo("abc") + " " + "Apple".compareTo("apple"));
        System.out.println("Apple".compareToIgnoreCase("apple") + " " + "a".compareToIgnoreCase("B"));
        System.out.println("HeLLo".equalsIgnoreCase("hello") + " " + "x".equalsIgnoreCase(null));
        System.out.println("con".concat("cat") + " " + "".concat(""));

        char[] chars = "hello".toCharArray();
        chars[0] = 'j';
        System.out.println(new String(chars) + " " + new String(chars, 1, 3) + " " + chars.length);
        System.out.println(String.valueOf(chars) + " " + String.copyValueOf(chars, 2, 2));
        System.out.println(String.valueOf(42) + String.valueOf(true) + String.valueOf('c') + String.valueOf(1.5));
        System.out.println(String.valueOf(10L) + " " + String.valueOf(2.5f) + " " + new String("copy") + new String());

        System.out.println("hello".hashCode() + " " + "".hashCode() + " " + "The quick brown fox".hashCode());
        System.out.println("é😀x".hashCode() + " " + "é😀x".length());

        // Text outside ASCII: indexes count UTF-16 units, so the emoji is two.
        String u = "né😀e";
        System.out.println(u.length() + " " + u.indexOf('e') + " " + u.indexOf("e") + " " + u.lastIndexOf('é'));
        System.out.println(u.substring(1, 2) + " " + u.substring(4) + " " + u.indexOf(0x1F600));
        System.out.println(u.toUpperCase() + " " + u.replace('e', 'E') + " " + u.contains("😀"));
        System.out.println(u.toCharArray().length + " " + u.startsWith("é", 1) + " " + u.endsWith("😀e"));

        Integer boxed = 5;
        String text = "5";
        System.out.println(text.equals(boxed) + " " + text.toString() + " " + text.intern());

        System.out.println("abc".substring(2, 1));
    }
}
