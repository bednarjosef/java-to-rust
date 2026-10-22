// A ClassCastException from inside a compareTo, where its bridge method casts the argument: a
// raw Comparable, an erased type variable bounded by Comparable, and a class of the program that
// implements Comparable. A try around the call catches it.
public class ComparableCastCaught {
    static class Version implements Comparable<Version> {
        final int major;

        Version(int major) {
            this.major = major;
        }

        public int compareTo(Version other) {
            return Integer.compare(major, other.major);
        }
    }

    static <T extends Comparable<T>> int cmp(T a, Object b) {
        return a.compareTo((T) b);
    }

    static int order(Comparable c, Object o) {
        return c.compareTo(o);
    }

    // The message up to its module clause: the clause of a class of the program names the
    // loader of a single file's run, which changes from one run to the next.
    static String head(ClassCastException e) {
        return e.getMessage().substring(0, e.getMessage().indexOf(" ("));
    }

    public static void main(String[] args) {
        Comparable c = "a";
        try {
            System.out.println(c.compareTo(Integer.valueOf(1)));
        } catch (ClassCastException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(cmp("b", 2));
        } catch (ClassCastException e) {
            System.out.println("generic " + e.getMessage());
        }
        try {
            System.out.println(cmp("b", new Object()));
        } catch (ClassCastException e) {
            System.out.println("bound " + e.getMessage());
        }
        try {
            System.out.println(order(new Version(1), "x"));
        } catch (ClassCastException e) {
            System.out.println("version " + head(e));
        }
        try {
            System.out.println(order(new Version(2), new Version(1)));
        } catch (ClassCastException e) {
            System.out.println("same class " + e.getMessage());
        }
    }
}
