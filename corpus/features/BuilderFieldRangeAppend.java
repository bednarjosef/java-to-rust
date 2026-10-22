// The same range append as `SharedBuilderRangeAppend`, in a program without threads: a builder
// field that may be null, appended to a range of a `CharSequence` that a program class implements,
// is a `StringBuilder` held as an `Rc`, and a null sequence appends a range of "null".
public class BuilderFieldRangeAppend {
    static class Digits implements CharSequence {
        final String text;

        Digits(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int i) {
            if (text.charAt(i) == '?') {
                throw new IllegalStateException("unknown digit at " + i);
            }
            return text.charAt(i);
        }

        public CharSequence subSequence(int start, int end) {
            return new Digits(text.substring(start, end));
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static class Column {
        StringBuilder out;

        void add(CharSequence s, int start, int end) {
            out.append(s, start, end).append('|');
        }

        void addPlain(CharSequence s, int start, int end) {
            out.append(s, start, end);
        }
    }

    static String run(Column column, CharSequence s, int start, int end) {
        try {
            column.add(s, start, end);
            return "ok " + column.out;
        } catch (IllegalStateException | IndexOutOfBoundsException e) {
            return e.getClass().getSimpleName() + ": " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        Column column = new Column();
        column.out = new StringBuilder();
        System.out.println(run(column, new Digits("12?4"), 0, 2));
        System.out.println(run(column, new Digits("12?4"), 1, 4));
        System.out.println(run(column, null, 0, 4));
        System.out.println(run(column, null, 2, 5));
        System.out.println(run(column, new Digits("56"), 1, 1));
        System.out.println(column.out.length());

        Column empty = new Column();
        try {
            empty.addPlain(new Digits("7"), 0, 1);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
    }
}
