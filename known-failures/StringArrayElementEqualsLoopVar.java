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
    }
}
