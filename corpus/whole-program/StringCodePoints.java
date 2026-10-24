// `String.codePoints()`: surrogate pairs as one code point, lone surrogates as their own, the
// stream's characteristics and size for a Latin-1 string and for any other, splitting it, and
// the exception for a null string.
import java.util.Arrays;
import java.util.Spliterator;
import java.util.stream.Collectors;

public class StringCodePoints {
    static void show(String s) {
        int[] points = s.codePoints().toArray();
        Spliterator.OfInt split = s.codePoints().spliterator();
        System.out.println(Arrays.toString(points) + " " + s.codePoints().count() + " "
            + split.characteristics() + " " + split.estimateSize() + " " + split.getExactSizeIfKnown());
    }

    public static void main(String[] args) {
        show("");
        show("abc");
        show("café");
        show("a😀b");
        show("😀😁");
        show("x\uD800");
        show("\uDC00y\uD800z");
        show("€1");

        String emoji = "ab😀cd😁";
        System.out.println(emoji.codePoints().mapToObj(Integer::toHexString).collect(Collectors.joining(",")));
        System.out.println(emoji.codePoints().filter(Character::isSupplementaryCodePoint).count());
        System.out.println(emoji.codePoints().parallel().sum() + " " + emoji.codePoints().sum());
        System.out.println(emoji.codePoints().map(Character::toUpperCase)
            .collect(StringBuilder::new, StringBuilder::appendCodePoint, StringBuilder::append));

        Spliterator.OfInt whole = "a😀".codePoints().spliterator();
        Spliterator.OfInt first = whole.trySplit();
        System.out.println((first == null) + " " + whole.estimateSize());
        Spliterator.OfInt pair = "😀".codePoints().spliterator();
        System.out.println(pair.trySplit() == null);
        Spliterator.OfInt longer = "abcd😀efgh".codePoints().spliterator();
        Spliterator.OfInt front = longer.trySplit();
        StringBuilder order = new StringBuilder();
        front.forEachRemaining((int c) -> order.appendCodePoint(c).append('|'));
        order.append("/");
        while (longer.tryAdvance((int c) -> order.appendCodePoint(c))) {
        }
        System.out.println(order + " " + longer.tryAdvance((int c) -> {}));

        String missing = args.length > 5 ? "x" : null;
        try {
            missing.codePoints();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
