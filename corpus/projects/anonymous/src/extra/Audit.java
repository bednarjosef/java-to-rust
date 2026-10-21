package extra;

/** Used only by methods that the program never calls. */
public class Audit {
    private static int records;

    public static String record(String name) {
        records++;
        return "audit " + name + " #" + records;
    }
}
