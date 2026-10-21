// A for-each over a string array that holds a null element: the loop variable may be null, and
// passed on as an `Object` it stays null.
public class ForEachNullStringAsObject {
    static StringBuilder seen = new StringBuilder();

    static void show(Object o) {
        seen.append(o).append(';');
        System.out.println("object " + o + " " + (o == null));
    }

    static String[] withHole(String a, String b) {
        return new String[] {a, null, b};
    }

    public static void main(String[] args) {
        String[] words = {"one", null, "three"};
        for (String w : words) {
            show(w);
        }
        for (String w : withHole("x", "z")) {
            show(w);
        }
        String[] plain = "p q".split(" ");
        for (String w : plain) {
            show(w);
        }
        System.out.println(seen);
    }
}
