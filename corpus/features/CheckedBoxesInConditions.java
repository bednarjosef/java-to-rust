// A `Boolean` that may be null, unboxed on the right of `&&` (`||`) once the left side has checked
// it against null, inside a `try` that catches `RuntimeException`: the check leaves nothing to
// throw there. As in javac's PrintingProcessor.
public class CheckedBoxesInConditions {
    static String line(Boolean newLine, Integer width) {
        StringBuilder sb = new StringBuilder();
        if (newLine != null && newLine) {
            sb.append("newline ");
        }
        if (width == null || width > 3) {
            sb.append("wide ");
        }
        if (width != null && newLine != null && !newLine) {
            sb.append("narrow ").append(width);
        }
        return sb.toString().trim();
    }

    public static void main(String[] args) {
        Boolean[] flags = {Boolean.TRUE, Boolean.FALSE, null};
        Integer[] widths = {1, 5, null};
        for (Boolean f : flags) {
            for (Integer w : widths) {
                try {
                    System.out.println("[" + line(f, w) + "]");
                } catch (RuntimeException e) {
                    System.out.println("caught " + e.getClass().getSimpleName());
                }
            }
        }
    }
}
