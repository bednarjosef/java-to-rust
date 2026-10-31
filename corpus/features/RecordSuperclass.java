// A record's superclass is `java.lang.Record`, whose superclass is `Object`: from the record's
// class literal and from the class of a record held as an `Object`.
public class RecordSuperclass {
    record Point(int x, int y) { }

    record Named(String name) implements Comparable<Named> {
        public int compareTo(Named other) {
            return name.compareTo(other.name);
        }
    }

    public static void main(String[] args) {
        System.out.println(Point.class.getSuperclass());
        System.out.println(Point.class.getSuperclass().getSuperclass());
        Object[] values = { new Point(1, 2), new Named("a"), "text" };
        for (Object v : values) {
            Class<?> s = v.getClass().getSuperclass();
            System.out.println(v.getClass().getSimpleName() + " " + s.getName() + " " + (s == Record.class));
        }
        System.out.println(new Point(3, 4).getClass().getSuperclass().getSimpleName());
    }
}
