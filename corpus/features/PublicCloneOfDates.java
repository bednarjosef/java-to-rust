// A public `clone()` of a class tree, as the JDK's CalendarDate has, whose copy is passed on more
// than once: the Rust code copies its handle too, which is not the Java method.
public class PublicCloneOfDates {
    abstract static class Date implements Cloneable {
        int day;

        Date(int day) { this.day = day; }

        public Object clone() {
            try {
                return super.clone();
            } catch (CloneNotSupportedException e) {
                throw new AssertionError(e);
            }
        }

        abstract String era();
    }

    static class GregorianDate extends Date {
        GregorianDate(int day) { super(day); }

        String era() { return "CE"; }
    }

    static class JulianDate extends Date {
        JulianDate(int day) { super(day); }

        String era() { return "AD"; }
    }

    static void normalize(Date d) { d.day = d.day % 7; }

    static String show(Date d) { return d.era() + " " + d.day; }

    static Date next(Date date, int days) {
        Date n = (Date) date.clone();
        n.day += days;
        normalize(n);
        Date[] both = {n, date};
        System.out.println(show(both[0]) + " after " + show(both[1]));
        return n;
    }

    public static void main(String[] args) {
        Date d = new GregorianDate(3);
        Date later = next(d, 10);
        System.out.println(show(next(later, 2)));
        System.out.println(show(next(new JulianDate(1), 1)));
    }
}
