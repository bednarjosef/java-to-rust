public class RecordNullComponentEquals {
    record Name(String text) {}

    record Entry(Name name, int count) {}

    public static void main(String[] args) {
        Entry a = new Entry(null, 1);
        Entry b = new Entry(null, 1);
        Entry c = new Entry(new Name("x"), 1);
        try {
            System.out.println(a.equals(b));
            System.out.println(a.equals(c));
            System.out.println(c.equals(a));
            System.out.println(c.equals(new Entry(new Name("x"), 1)));
            System.out.println(a.hashCode() == b.hashCode());
            Entry n = args.length > 5 ? a : null;
            System.out.println(n.hashCode());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
