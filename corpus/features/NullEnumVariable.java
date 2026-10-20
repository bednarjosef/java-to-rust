public class NullEnumVariable {
    enum Level { LOW, HIGH }

    public static void main(String[] args) {
        Level best = null;
        System.out.println(best == null ? "none" : "some");
        best = Level.HIGH;
        System.out.println(best);
        if (best != null) {
            System.out.println("set " + best);
        }
        best = null;
        System.out.println("now " + best);
        if (best == null) {
            System.out.println("null again");
        }
        Level last = null;
        for (int i = 0; i < 3; i++) {
            if (i % 2 == 0) {
                last = Level.LOW;
            } else {
                last = null;
            }
            System.out.println(i + " " + last);
        }
        System.out.println(last != null);
    }
}
