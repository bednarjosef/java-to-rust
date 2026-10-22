// `valueOf` of an enum given a name that may be null: null throws `NullPointerException` with
// `Enum.valueOf`'s message, which a `catch` sees.
public class EnumValueOfNullName {
    enum State { INIT, GENERATE }

    static String option(String key) { return key.length() > 3 ? null : key.equals("gen") ? "GENERATE" : "INIT"; }

    public static void main(String[] args) {
        try {
            System.out.println(State.valueOf(option("abc")));
            System.out.println(State.valueOf(option("abcd")));
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            System.out.println(Enum.valueOf(State.class, option("abcd")));
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            State s = State.valueOf(option("abcd"));
            System.out.println(s);
        } catch (RuntimeException e) {
            System.out.println("caught " + e.getClass().getName());
        }
        System.out.println(State.valueOf(option("gen")));
    }
}
