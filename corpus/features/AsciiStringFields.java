// String fields and statics that only ever hold ASCII text are indexed by byte (length,
// substring), and those that may hold other text count UTF-16 units: a field assigned other text
// anywhere, an enum's field set from its constants, a runtime exception's message, the result of
// a generic method. A field read where the statement also changes its object, or the field, is
// copied before the change, as Java reads it.
public class AsciiStringFields {
    static final String TEXT = "[ab,cd],[e]";
    static String mode = "plain";

    enum Greeting {
        SHORT("ahoj"),
        LONG("dobrý den");

        final String text;

        Greeting(String text) {
            this.text = text;
        }
    }

    static class Scanner {
        private final String input;
        private int index = -1;
        private String current;

        Scanner(String input) {
            this.input = input;
        }

        void read() {
            index++;
            if (index < input.length()) {
                current = input.substring(index, index + 1);
            } else {
                current = null;
            }
        }

        int countCommas() {
            int commas = 0;
            read();
            while (current != null) {
                if (current.equals(",")) {
                    commas++;
                }
                read();
            }
            return commas;
        }

        String rest() {
            return input.substring(Math.min(index, input.length()));
        }

        String advance() {
            read();
            return current == null ? "end" : current;
        }

        String pair() {
            // `input` is read with the old index, before `advance` moves it.
            return join(input.substring(index + 1), advance()) + input.length();
        }
    }

    static class Label {
        String text;
        int changes;

        Label(String text) {
            this.text = text;
        }

        String grow() {
            text = text + "+";
            changes++;
            return text;
        }

        String both() {
            // The first operand is read before `grow` changes the field.
            return join(text, grow()) + " " + text.length();
        }

        void shift() {
            text = text.substring(1) + text.substring(0, 1);
        }
    }

    static String join(String a, String b) {
        return a + "|" + b;
    }

    static int lengthAndName(int n, Label label) {
        return n + label.text.length();
    }

    static <T> T pick(boolean first, T a, T b) {
        return first ? a : b;
    }

    public static void main(String[] args) {
        Scanner s = new Scanner(TEXT);
        System.out.println(s.countCommas() + " " + s.rest().length());
        Scanner t = new Scanner("xyz");
        System.out.println(t.pair() + " " + t.pair() + " " + t.pair());

        Label plain = new Label("abc");
        System.out.println(plain.both());
        plain.shift();
        System.out.println(plain.text + " " + plain.text.length() + " " + plain.text.substring(1, 3));
        System.out.println(lengthAndName(plain.text.length(), plain));
        Label wide = new Label("x");
        wide.text = "žluť" + wide.text;
        System.out.println(wide.text.length() + " " + wide.text.substring(2) + " " + wide.both());

        for (Greeting g : Greeting.values()) {
            System.out.println(g + " " + g.text.length() + " " + g.text.substring(g.text.length() - 3));
        }

        try {
            Integer.parseInt("č12");
        } catch (NumberFormatException e) {
            String message = e.getMessage();
            System.out.println(message.length() + " " + message.substring(message.length() - 5));
        }

        String picked = pick(true, "čaj", "tea");
        System.out.println(picked.length() + " " + picked.substring(1));

        System.out.println(mode.length() + " " + mode.substring(2));
        mode = "ñ" + mode;
        System.out.println(mode.length() + " " + mode.substring(2));

        Label face = new Label("a😀b");
        System.out.println(face.text.length() + " " + face.text.substring(3));
    }
}
