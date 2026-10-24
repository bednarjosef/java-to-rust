// Integer.parseInt, Long.parseLong and the unsigned forms over a range of a CharSequence that is
// not a string: the sequence sees each call of length, charAt and subSequence, in Java's order,
// and each failure has Java's exception and message.
public class ParseCharSequenceRanges {
    static final class Traced implements CharSequence {
        private final String text;
        private final StringBuilder log;

        Traced(String text, StringBuilder log) {
            this.text = text;
            this.log = log;
        }

        public int length() {
            log.append('L');
            return text.length();
        }

        public char charAt(int index) {
            log.append(index);
            return text.charAt(index);
        }

        public CharSequence subSequence(int start, int end) {
            log.append('S');
            return new Traced(text.substring(start, end), log);
        }

        public String toString() {
            log.append('T');
            return text;
        }
    }

    interface Parse {
        long run(CharSequence s, int begin, int end, int radix);
    }

    static void attempt(String label, Parse parse, String text, int begin, int end, int radix) {
        StringBuilder log = new StringBuilder();
        String result;
        try {
            result = Long.toString(parse.run(new Traced(text, log), begin, end, radix));
        } catch (RuntimeException e) {
            result = e.getClass().getName() + ": " + e.getMessage();
        }
        System.out.println(label + " \"" + text + "\" [" + begin + ", " + end + ") " + radix + ": " + result
                + " | " + log);
    }

    public static void main(String[] args) {
        Parse parseInt = (s, b, e, r) -> Integer.parseInt(s, b, e, r);
        Parse parseLong = (s, b, e, r) -> Long.parseLong(s, b, e, r);
        Parse unsignedInt = (s, b, e, r) -> Integer.parseUnsignedInt(s, b, e, r);
        Parse unsignedLong = (s, b, e, r) -> Long.parseUnsignedLong(s, b, e, r);
        Parse[] all = {parseInt, parseLong, unsignedInt, unsignedLong};
        String[] names = {"int", "long", "uint", "ulong"};
        String[][] cases = {
            {"x123y", "1", "4", "10"},
            {"-2147483648", "0", "11", "10"},
            {"2147483648", "0", "10", "10"},
            {"-2147483649", "0", "11", "10"},
            {"4294967295", "0", "10", "10"},
            {"4294967296", "0", "10", "10"},
            {"9223372036854775807", "0", "19", "10"},
            {"-9223372036854775808", "0", "20", "10"},
            {"9223372036854775808", "0", "19", "10"},
            {"18446744073709551615", "0", "20", "10"},
            {"18446744073709551616", "0", "20", "10"},
            {"+7f", "0", "3", "16"},
            {"-", "0", "1", "10"},
            {"+", "0", "1", "10"},
            {"12a4", "0", "4", "10"},
            {"a", "0", "1", "10"},
            {"zz", "0", "2", "36"},
            {"abc", "1", "1", "10"},
            {"abc", "1", "1", "16"},
            {"abc", "2", "5", "10"},
            {"12", "0", "2", "1"},
            {"12", "0", "2", "37"},
            {"-5", "0", "2", "10"},
        };
        for (String[] c : cases) {
            for (int i = 0; i < all.length; i++) {
                attempt(names[i], all[i], c[0], Integer.parseInt(c[1]), Integer.parseInt(c[2]),
                        Integer.parseInt(c[3]));
            }
        }
        try {
            Integer.parseInt(null, 0, 1, 10);
        } catch (NullPointerException e) {
            System.out.println("null: " + e.getMessage());
        }
        CharSequence text = args.length == 0 ? "--42--" : new Traced("--42--", new StringBuilder());
        System.out.println(Integer.parseInt(text, 2, 4, 10) + Long.parseLong("x-9", 1, 3, 10));
    }
}
