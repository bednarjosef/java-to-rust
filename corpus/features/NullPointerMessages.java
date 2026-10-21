// The message of a NullPointerException that the program creates, read where it is caught.
public class NullPointerMessages {
    static String check(String value, String what) {
        if (value == null) {
            throw new NullPointerException(what + " is null");
        }
        return value;
    }

    public static void main(String[] args) {
        try {
            System.out.println(check("present", "first"));
            System.out.println(check(null, "second"));
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            throw new NullPointerException();
        } catch (RuntimeException e) {
            System.out.println("caught: " + e.getMessage() + " " + e.getLocalizedMessage());
        }
    }
}
