// `seen[k].equals(w)` in a loop over a String[], with `w` the for-each variable and `k` bounded
// by a mutable local, so the element is read through a bounds-checked copy that the comparison
// borrows. The first pass never enters the inner loop; the second fills `seen` as it goes.
public class StringArrayElementEqualsLoopVar {
    public static void main(String[] args) {
        String[] words = {"a", "b", "a"};
        String[] seen = new String[3];
        int distinct = 0;
        for (String w : words) {
            for (int k = 0; k < distinct; k++) {
                if (seen[k].equals(w)) {
                    System.out.println("again " + w);
                }
            }
        }
        System.out.println("distinct " + distinct);
        for (String w : words) {
            boolean found = false;
            for (int k = 0; k < distinct; k++) {
                if (seen[k].equals(w)) {
                    System.out.println("again " + w);
                    found = true;
                }
            }
            if (!found) {
                seen[distinct++] = w;
            }
        }
        System.out.println("distinct " + distinct + " " + seen[0] + seen[1] + seen[2]);
    }
}
