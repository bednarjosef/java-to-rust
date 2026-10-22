public class CompareToNull {
    static class Version implements Comparable<Version> {
        final int major;

        Version(int major) {
            this.major = major;
        }

        public int compareTo(Version other) {
            return Integer.compare(major, other.major);
        }
    }

    static int order(Comparable c, Object o) {
        return c.compareTo(o);
    }

    public static void main(String[] args) {
        Comparable c = "a";
        try {
            System.out.println(c.compareTo(null));
        } catch (NullPointerException e) {
            System.out.println("raw " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            System.out.println("a".compareTo(null));
        } catch (NullPointerException e) {
            System.out.println("string " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            System.out.println(Integer.valueOf(1).compareTo(null));
        } catch (NullPointerException e) {
            System.out.println("integer " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            System.out.println(new Version(1).compareTo(null));
        } catch (NullPointerException e) {
            System.out.println("version " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            System.out.println(order(new Version(1), null));
        } catch (NullPointerException e) {
            System.out.println("bridge " + e.getClass().getName() + " " + e.getMessage());
        }
        try {
            System.out.println(order("b", null));
        } catch (NullPointerException e) {
            System.out.println("raw string " + e.getClass().getName() + " " + e.getMessage());
        }
        System.out.println(order(new Version(1), new Version(2)));
    }
}
