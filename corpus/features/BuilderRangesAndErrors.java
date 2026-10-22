// `append(CharSequence, int, int)` appends a range of a string, a builder or a null (as "null"), and
// throws an IndexOutOfBoundsException with Java 26's message for a range out of bounds. The
// no-argument LinkageError has neither a message nor a cause.
public class BuilderRangesAndErrors {
    public static void main(String[] args) {
        CharSequence text = "hello, world";
        CharSequence builder = new StringBuilder("abcdef");
        StringBuilder sb = new StringBuilder("[");
        sb.append(text, 7, 12).append('|');
        sb.append(builder, 1, 3).append(']');
        System.out.println(sb);

        CharSequence nothing = null;
        sb.setLength(0);
        // Room for more leaves the text as it is; a capacity that is not positive asks for none.
        sb.ensureCapacity(100);
        sb.ensureCapacity(-5);
        System.out.println("[" + sb + "] " + sb.length());
        sb.append(nothing, 0, 2).append(nothing, 1, 4);
        System.out.println(sb);

        StringBuffer buffer = new StringBuffer("xy");
        buffer.append(text, 0, 5).append(builder, 4, 6);
        System.out.println(buffer);

        int[][] ranges = {{-1, 2}, {3, 2}, {0, 13}, {4, 14}};
        for (int[] range : ranges) {
            try {
                sb.append(text, range[0], range[1]);
                System.out.println("no exception");
            } catch (IndexOutOfBoundsException e) {
                System.out.println(e.getMessage() + " / " + e.getCause());
            }
        }

        try {
            buffer.append(builder, 2, 7);
        } catch (IndexOutOfBoundsException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }

        LinkageError linkage = new LinkageError();
        System.out.println(linkage.getMessage() + " " + linkage.getCause());
        System.out.println(sb.length());
    }
}
