// Scans a text one character at a time, as a hand-written parser does: each character is taken
// with substring(index, index + 1) from a string field and compared as a one-letter string, and
// the length is read at every step. The text is a constant, so it is ASCII. A translation whose
// length() or substring() walks the whole text, or copies it, makes this quadratic.
public class TokenScanner {
    static final String TEXT = "{\"items\":[{\"id\":101,\"name\":\"alpha\",\"tags\":[\"x\",\"y\"],\"price\":12.50},"
            + "{\"id\":102,\"name\":\"beta\",\"tags\":[],\"price\":7.25},{\"id\":103,\"name\":\"gamma\","
            + "\"tags\":[\"z\"],\"price\":19.99},{\"id\":104,\"name\":\"delta\",\"tags\":[\"x\"],\"price\":3.10},"
            + "{\"id\":105,\"name\":\"epsilon\",\"tags\":[\"y\",\"z\"],\"price\":42.00}],\"count\":5,"
            + "\"note\":\"a list of five items with ids, names, tags and prices, written out by hand\"}";

    private final String input;
    private int index;
    private String current;
    private int strings;
    private int numbers;
    private int digits;
    private int punctuation;

    TokenScanner(String input) {
        this.input = input;
        this.index = -1;
    }

    private void read() {
        index++;
        if (index < input.length()) {
            current = input.substring(index, index + 1);
        } else {
            current = null;
        }
    }

    private boolean isDigit() {
        return current != null && "0123456789".contains(current);
    }

    void scan() {
        read();
        while (current != null) {
            if (current.equals("\"")) {
                read();
                while (!current.equals("\"")) {
                    read();
                }
                strings++;
                read();
            } else if (isDigit()) {
                while (isDigit() || ".".equals(current)) {
                    if (isDigit()) {
                        digits++;
                    }
                    read();
                }
                numbers++;
            } else {
                punctuation++;
                read();
            }
        }
    }

    public static void main(String[] args) {
        // The text repeated, so that a pass over it is long: a constant of about 40 KB.
        String text = TEXT + TEXT + TEXT + TEXT + TEXT + TEXT + TEXT + TEXT;
        text = text + text + text + text + text + text + text + text + text + text;
        long total = 0;
        for (int round = 0; round < 4000; round++) {
            TokenScanner s = new TokenScanner(text);
            s.scan();
            total += s.strings * 7L + s.numbers * 3L + s.digits + s.punctuation;
        }
        System.out.println("length " + text.length() + " total " + total);
    }
}
