// A program whose strings are shared (`==` compares their objects) that switches on strings, calls
// `toString` on a string that may be null, and passes a parameter to a method after a field took
// it while another field's initializer runs first.
public class SharedStringsInSwitchesAndConstructors {
    private static int counter;

    private final String name;
    private final int length;
    private final int order = count();
    private String mode;

    private SharedStringsInSwitchesAndConstructors(String name) {
        this.name = name;
        this.length = lengthOf(name);
    }

    private static int count() {
        counter++;
        if (counter > 100) {
            throw new IllegalStateException("too many");
        }
        return counter;
    }

    private static int lengthOf(String name) {
        if (name != null && name.isEmpty()) {
            throw new IllegalStateException("empty name");
        }
        return name == null ? -1 : name.length();
    }

    static String kind(String s) {
        if (s != null) {
            switch (s) {
                case "a":
                    return "A";
                case "b":
                    return "B";
                default:
                    return "other";
            }
        }
        return "none";
    }

    String modeText() {
        return switch (mode) {
            case "fast" -> "F";
            case "slow" -> "S";
            default -> "?";
        };
    }

    static String trimmed(String s) {
        return s == null ? null : s.trim();
    }

    String shown() {
        return trimmed(name).toString();
    }

    public static void main(String[] args) {
        SharedStringsInSwitchesAndConstructors x = new SharedStringsInSwitchesAndConstructors(" x ");
        System.out.println(x.order + " " + x.length + " [" + x.shown() + "]");
        System.out.println(kind("a") + kind("b") + kind("z") + kind(null));
        x.mode = "slow";
        System.out.println(x.modeText());
        x.mode = null;
        try {
            System.out.println(x.modeText());
        } catch (NullPointerException e) {
            System.out.println("null mode");
        }
        try {
            new SharedStringsInSwitchesAndConstructors("");
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        SharedStringsInSwitchesAndConstructors none = new SharedStringsInSwitchesAndConstructors(null);
        System.out.println(none.order + " " + none.length);
        try {
            System.out.println(none.shown());
        } catch (NullPointerException e) {
            System.out.println("null name");
        }
        String made = new String("q");
        System.out.println(made.trim() == made);
    }
}
