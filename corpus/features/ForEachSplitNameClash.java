// A for-each over `split` and a later local of the same name in the same method: the pieces are
// never null, even though the later `w` can be. Passing the loop variable on as an `Object` must
// not wrap it in an `Option`, and the local must still print null.
public class ForEachSplitNameClash {
    static Object[] items = new Object[8];
    static int count;

    static void add(Object o) {
        items[count++] = o;
    }

    static String maybe(boolean yes) {
        return yes ? "yes" : null;
    }

    static String run(String text) {
        String out = "";
        for (String w : text.split(" ")) {
            add(w);
            out = out + "[" + w + "]";
        }
        String w = maybe(count % 2 == 0);
        add(w);
        return out + " " + w;
    }

    static String joined(String text) {
        String[] parts = {text, "x"};
        String out = "";
        for (String w : parts) {
            out = out + "<" + w + ">";
        }
        int k = 0;
        for (String w : parts) {
            parts[k] = w + "!";
            k++;
        }
        String w = maybe(parts.length > 1);
        add(w);
        return out + " " + parts[0] + parts[1] + " " + w;
    }

    public static void main(String[] args) {
        System.out.println(run("a b"));
        System.out.println(run("c d e"));
        System.out.println(joined("f g"));
        for (int i = 0; i < count; i++) {
            System.out.println(i + " " + items[i] + " " + (items[i] == null));
        }
    }
}
