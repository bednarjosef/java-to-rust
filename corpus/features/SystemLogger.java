// System.getLogger(name).log(level, message): the default logger prints INFO and above to
// System.err, and below that evaluates the message and prints nothing.
public class SystemLogger {
    static int built = 0;

    static String message(String text) {
        built++;
        return text;
    }

    public static void main(String[] args) {
        System.getLogger(SystemLogger.class.getName()).log(System.Logger.Level.INFO, message("shown"));
        System.getLogger("quiet").log(System.Logger.Level.DEBUG, message("hidden"));
        System.getLogger("loud").log(System.Logger.Level.WARNING, "careful " + built);
        System.out.println("messages built: " + built);
    }
}
