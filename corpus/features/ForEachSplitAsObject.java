// The loop variable of a for-each over `split` is a plain string, never null, so passing it on as
// an `Object` needs no null check: `split` never yields a null piece.
public class ForEachSplitAsObject {
    static Object[] items = new Object[8];
    static int count;
    // A string array that holds null: its elements are nullable, which must not make the pieces
    // of `split` nullable too.
    static String[] holes = new String[2];

    static void add(Object o) {
        items[count++] = o;
    }

    static String reverseWords(String text) {
        String out = "";
        for (String w : text.split(" ")) {
            add(w);
            out = w + (out.isEmpty() ? "" : " ") + out;
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(reverseWords("the  quick brown fox"));
        for (int i = 0; i < count; i++) {
            System.out.println(i + " " + items[i] + " " + (items[i] == null));
        }
        int n = 0;
        for (String piece : "a,,b,".split(",")) {
            add(piece);
            n += piece.length();
        }
        System.out.println(count + " " + n + " " + items[count - 1]);
    }
}
