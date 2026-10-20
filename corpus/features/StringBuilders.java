// StringBuilder: appending every kind of value, chaining, inserting, deleting, replacing,
// reversing (surrogate pairs stay in order), and builders shared between variables, fields and
// parameters, which all see the same text. The last line deletes past the end, which throws
// StringIndexOutOfBoundsException.
public class StringBuilders {
    enum Suit { HEARTS, SPADES }

    static class Point {
        int x, y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        public String toString() {
            return "(" + x + ", " + y + ")";
        }
    }

    static class Log {
        private final StringBuilder lines = new StringBuilder();
        private int count;

        void add(String line) {
            if (count > 0) {
                lines.append('\n');
            }
            lines.append(count).append(": ").append(line);
            count++;
        }

        String text() {
            return lines.toString();
        }
    }

    static void fill(StringBuilder sb, int n) {
        for (int i = 0; i < n; i++) {
            sb.append(i);
        }
    }

    static StringBuilder bracket(StringBuilder sb) {
        return sb.insert(0, '[').append(']');
    }

    static String csv(int[] values) {
        StringBuilder sb = new StringBuilder();
        for (int v : values) {
            if (sb.length() > 0) {
                sb.append(',');
            }
            sb.append(v);
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        String nothingYet = args.length > 5 ? "set" : null;
        StringBuilder sb = new StringBuilder();
        sb.append("text").append(' ').append(42).append(' ').append(7L).append(' ').append(true);
        sb.append(' ').append(1.5).append(' ').append(2.5f).append(' ').append(1e21).append(' ').append(nothingYet == null);
        System.out.println(sb);
        System.out.println(sb.length() + " " + sb.charAt(0) + " " + sb.indexOf("42") + " " + sb.lastIndexOf(" "));

        StringBuilder other = new StringBuilder("abc");
        other.append(Suit.SPADES).append(Suit.valueOf("HEARTS")).append(new Point(1, 2)).append(new char[] {'x', 'y'});
        Integer boxed = 9;
        String nothing = null;
        other.append(boxed).append(nothing).append("abcdef", 1, 3).append(new char[] {'p', 'q', 'r'}, 1, 2);
        System.out.println(other + " " + other.length());

        StringBuilder alias = other;
        alias.setLength(3);
        System.out.println(other + " " + (alias == other) + " " + (alias != sb));
        other.append(other);
        System.out.println(alias);

        StringBuilder edit = new StringBuilder("Hello World");
        edit.insert(5, ",").insert(0, 1).insert(edit.length(), '!');
        System.out.println(edit);
        edit.delete(0, 1).deleteCharAt(5).replace(0, 5, "Howdy").setCharAt(0, 'h');
        System.out.println(edit + " " + edit.substring(6) + " " + edit.substring(0, 3));
        edit.replace(6, 100, "there");
        edit.delete(edit.length() - 1, edit.length() + 10);
        System.out.println(edit.reverse() + " " + edit.isEmpty());

        StringBuilder pairs = new StringBuilder("a😀bé");
        System.out.println(pairs.reverse() + " " + pairs.length());
        pairs.setLength(6);
        System.out.println("[" + pairs.toString().replace('\0', '0') + "]");

        StringBuilder filled = new StringBuilder(16);
        fill(filled, 5);
        System.out.println(filled + " " + bracket(filled) + " " + filled);
        System.out.println(csv(new int[] {3, 1, 4, 1, 5}) + " [" + csv(new int[0]) + "]");

        Log log = new Log();
        log.add("start");
        log.add("middle");
        log.add("end");
        System.out.println(log.text());

        StringBuilder order = new StringBuilder("x");
        String seen = "before " + order + " after " + order.append("y") + " " + order;
        System.out.println(seen);
        System.out.println(new StringBuilder("abc").compareTo(new StringBuilder("abd")));
        String built = new StringBuilder().append("chain").append(1).toString();
        System.out.println(built + " " + String.valueOf(order) + " " + new String(order));

        StringBuilder self = new StringBuilder("ab");
        self.append(self.length()).append(self).insert(1, self.charAt(0));
        System.out.println(self);

        new StringBuilder("abc").delete(4, 5);
    }
}
