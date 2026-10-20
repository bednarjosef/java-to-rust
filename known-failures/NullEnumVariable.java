public class NullEnumVariable {
    enum Level { LOW, HIGH }

    public static void main(String[] args) {
        Level best = null;
        System.out.println(best == null ? "none" : "some");
        best = Level.HIGH;
        System.out.println(best);
    }
}
