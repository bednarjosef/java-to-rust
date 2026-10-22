// An enum whose constructor throws for one constant: the first use of the enum fails with
// `ExceptionInInitializerError`, which a `catch` sees, and a later use with
// `NoClassDefFoundError`.
public class EnumConstructorThrowsCaught {
    enum Level {
        LOW(1),
        HIGH(10),
        BROKEN(-1);

        final int weight;
        final String label;

        Level(int weight) {
            System.out.println("building " + weight);
            if (weight < 0) {
                throw new IllegalStateException("negative weight " + weight);
            }
            this.weight = weight;
            this.label = "level" + weight;
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(Level.HIGH.label);
        } catch (ExceptionInInitializerError e) {
            System.out.println("first use: " + e.getCause().getMessage());
        }
        try {
            System.out.println(Level.LOW.weight);
        } catch (NoClassDefFoundError e) {
            System.out.println("second use failed too");
        }
        System.out.println("done");
    }
}
