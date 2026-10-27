// `StringBuilder.capacity()` and `trimToSize()`, which log4j's `StringBuilders.trimToMaxSize`
// calls on every builder it reuses: the capacity grows as Java grows it, including when text
// beyond Latin-1 first arrives in a builder Java still keeps one byte per character.
public class BuilderCapacity {
    static void show(String what, StringBuilder sb) {
        System.out.println(what + ": " + sb.length() + "/" + sb.capacity());
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        show("new", sb);
        sb.append("0123456789abcdefg");
        show("17 chars", sb);
        sb.append('x').append(12345).append(2.5).append(true);
        show("more", sb);
        sb.append("é€ and more text to grow it past seventy units of room, at least");
        show("long", sb);
        sb.setLength(5);
        show("cut", sb);
        sb.trimToSize();
        show("trimmed", sb);
        sb.ensureCapacity(7);
        show("ensure 7", sb);
        sb.ensureCapacity(100);
        show("ensure 100", sb);
        sb.setLength(0);
        sb.trimToSize();
        show("empty trimmed", sb);
        sb.insert(0, "abc");
        show("insert", sb);
        sb.replace(0, 1, "0123456789");
        show("replace", sb);
        sb.repeat('z', 40);
        show("repeat", sb);
        sb.setLength(300);
        show("setLength 300", sb);
        StringBuilder text = new StringBuilder("hello");
        show("from text", text);
        StringBuilder sized = new StringBuilder(3);
        sized.append("abcd");
        show("sized 3 + 4", sized);
        sized.appendCodePoint(0x1F600);
        show("code point", sized);
        // log4j's StringBuilders.trimToMaxSize
        StringBuilder big = new StringBuilder();
        for (int i = 0; i < 100; i++) {
            big.append("0123456789");
        }
        if (big.capacity() > 518) {
            big.setLength(518);
            big.trimToSize();
        }
        show("trimmed to 518", big);
    }
}
