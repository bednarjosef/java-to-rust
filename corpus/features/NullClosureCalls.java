// Calls of a functional interface whose value may be null and is neither a variable nor a
// field of an object: an enum's field, which the translation keeps as a table of the constants'
// values (read in the enum's own code too), an element of an array, and a method's return
// value. Null throws Java's `NullPointerException` there, which a `catch` reads.
public class NullClosureCalls {
    interface Affector {
        boolean affect(int pos);
    }

    interface Filter {
        Integer filter(int r, int pos);
    }

    enum Mode {
        REPLACE(p -> false, (r, p) -> r),
        OUTLINE(p -> p > 1, (r, p) -> p != r ? null : r),
        BROKEN(null, null);

        final Affector affector;
        final Filter filter;

        Mode(Affector affector, Filter filter) {
            this.affector = affector;
            this.filter = filter;
        }

        boolean affects(int pos) {
            return this.affector.affect(pos);
        }
    }

    static int fill(Mode mode, int n) {
        int count = 0;
        for (int pos = 0; pos < n; pos++) {
            if (mode.affector.affect(pos)) {
                count += 10;
            }
            Integer block = mode.filter.filter(2, pos);
            if (block != null) {
                count += block;
            }
        }
        return count;
    }

    static Affector[] affectors = { p -> p == 0, null };

    static Affector pick(int i) {
        return i < 0 ? null : affectors[i];
    }

    public static void main(String[] args) {
        for (Mode m : Mode.values()) {
            try {
                System.out.println(m + " " + fill(m, 4));
            } catch (NullPointerException e) {
                System.out.println(m + " " + e.getMessage());
            }
            try {
                System.out.println(m.affects(2));
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
        for (int i = 0; i < 2; i++) {
            try {
                System.out.println(affectors[i].affect(0));
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
        for (int i = -1; i < 2; i++) {
            try {
                System.out.println(pick(i).affect(1));
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
    }
}
