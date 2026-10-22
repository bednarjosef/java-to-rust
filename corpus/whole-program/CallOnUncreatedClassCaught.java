// A call on a field of a class that the program never creates, so the field is null, whose
// result becomes a value of a supertype: the call throws `NullPointerException`, which a
// `catch` sees.
public class CallOnUncreatedClassCaught {
    static abstract class Calendar {
        static abstract class Date { int day; }
        abstract Date newDate(String zone);
    }

    static class Julian extends Calendar {
        static class Date extends Calendar.Date { }
        Date newDate(String zone) { Date d = new Date(); d.day = zone.length(); return d; }
    }

    static class Gregorian extends Calendar {
        static class Date extends Calendar.Date { }
        Date newDate(String zone) { Date d = new Date(); d.day = 1 + zone.length(); return d; }
    }

    static Julian julian;
    static Gregorian gregorian = new Gregorian();
    Calendar.Date date;

    void compute(boolean old) {
        if (old) {
            date = julian.newDate("zone");
        } else {
            date = gregorian.newDate("z");
        }
    }

    static Calendar.Date first(Calendar.Date[] dates) { return dates[0]; }

    public static void main(String[] args) {
        CallOnUncreatedClassCaught c = new CallOnUncreatedClassCaught();
        try {
            c.compute(false);
            System.out.println(c.date.day);
            c.compute(true);
            System.out.println(c.date.day);
        } catch (NullPointerException e) {
            System.out.println("caught");
        }
        try {
            System.out.println(first(new Calendar.Date[] { julian.newDate("x") }).day);
        } catch (NullPointerException e) {
            System.out.println("caught again");
        }
    }
}
