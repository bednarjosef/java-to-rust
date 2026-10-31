// A string static is read where it is, not copied, by a statement that cannot change it: one
// that assigns no static and runs no other Java code. A statement that assigns the static, calls
// a method that does, starts another class's initialization that does, or concatenates an object
// whose toString does, reads the value the static has at that point, as Java does.
public class StaticStringsReadInPlace {
    static String text = "alpha beta gamma delta";
    static String other;
    static int changes;

    static class Late {
        static final int VALUE;

        static {
            text = "changed by Late";
            VALUE = 7;
        }
    }

    static class Changer {
        @Override
        public String toString() {
            text = "changed by toString";
            return "<changer>";
        }
    }

    static int change() {
        changes++;
        text = "changed by a call " + changes;
        return 1;
    }

    static long walk() {
        long total = 0;
        int from = 0;
        for (int i = text.indexOf(' '); i >= 0; i = text.indexOf(' ', from)) {
            String word = text.substring(from, i);
            total += word.length() * 31 + text.charAt(from) + text.lastIndexOf(' ', i - 1);
            from = i + 1;
        }
        return total + text.length() + text.substring(from).length();
    }

    public static void main(String[] args) {
        System.out.println(walk());
        text = "čaj a káva, x😀y a \ud800 konec";
        System.out.println(walk());
        text = "one two three";

        // The statement assigns the static it reads.
        text = text.substring(4) + " " + text.charAt(0);
        System.out.println(text);
        text += "!" + text.length();
        System.out.println(text);

        // A call in the statement changes the static after it is read.
        int before = text.length() + change() + text.length();
        System.out.println(before + " " + text);
        String joined = text.substring(0, 6) + change() + text.substring(0, 6);
        System.out.println(joined);

        // Another class's initialization changes it.
        int late = text.length() * 100 + Late.VALUE + text.length();
        System.out.println(late + " " + text);

        // A concatenated object's toString changes it.
        Object changer = new Changer();
        text = "before";
        String concat = text + changer + text;
        System.out.println(concat);

        // Another static is assigned from it, in place.
        other = text.substring(2);
        System.out.println(other + " " + text.indexOf("ng"));

        // Two statements, the second changing the static.
        String head = text.substring(0, 3);
        text = "after";
        System.out.println(head + " " + text);

        // A lambda reads it when it runs.
        java.util.function.IntSupplier length = () -> text.length();
        text = "longer text";
        System.out.println(length.getAsInt());

        // Null.
        text = null;
        try {
            System.out.println(text.length());
        } catch (NullPointerException e) {
            System.out.println("null static");
        }
        System.out.println(text == null ? "still null" : text);
    }
}
