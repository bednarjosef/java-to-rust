// A pattern variable in scope after `if (!(x instanceof T name)) throw ..;`, as in
// `JapaneseChronology.date(Era, ..)`: the rest of the block runs only when the pattern matched.

public class PatternThenThrow {
    interface Era {}

    record Named(String name) implements Era {}

    record Numbered(int number) implements Era {}

    static String date(Era era, int year) {
        if (!(era instanceof Named named)) {
            throw new ClassCastException("Era must be Named");
        }
        return named.name() + " " + year;
    }

    static int checked(Object value) throws Exception {
        if (!(value instanceof Integer n)) {
            throw new Exception("not a number: " + value);
        }
        return n * 2;
    }

    public static void main(String[] args) {
        System.out.println(date(new Named("Reiwa"), 6));
        try {
            System.out.println(date(new Numbered(3), 6));
        } catch (ClassCastException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            System.out.println(checked(21));
            System.out.println(checked("x"));
        } catch (Exception e) {
            System.out.println("caught: " + e.getMessage());
        }
        System.out.println(date(new Named("Heisei"), 31));
    }
}
