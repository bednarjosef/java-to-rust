// `new StringBuilder(CharSequence)` and `new StringBuffer(CharSequence)` start with the characters
// of a string or a builder; a null one throws a NullPointerException whose message names `seq`.
public class BuilderFromTextAndZone {
    public static void main(String[] args) {
        CharSequence text = "hello, world";
        CharSequence builder = new StringBuilder("abcdef");

        StringBuilder sb = new StringBuilder(text);
        sb.append('|').append(builder).append('|');
        System.out.println(sb + " " + sb.length());

        StringBuilder copy = new StringBuilder(sb);
        sb.append("!");
        System.out.println(copy + " / " + sb);

        StringBuffer buffer = new StringBuffer(builder);
        buffer.insert(0, '<').append('>');
        System.out.println(buffer + " " + buffer.length());

        CharSequence nothing = null;
        try {
            new StringBuilder(nothing);
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }

        try {
            StringBuffer none = new StringBuffer(nothing);
            System.out.println(none);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }

        System.out.println(new StringBuilder(text).reverse());
    }
}
