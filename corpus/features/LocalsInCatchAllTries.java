// Locals declared without a value before a try with a catch of Throwable or Exception, which the
// translation runs as a closure, and assigned in its body: used only there (in a loop, in a
// while condition, of a type with no value to start as), also assigned after it, read after it,
// set before it and in it from its own value (`s = f(s)`), and set in each case of a switch
// that then reads it, as javax.naming's SearchFilter.createNextFilter does.
public class LocalsInCatchAllTries {
    static int parse(String text) {
        int i, c, limit;
        StringBuilder digits;
        try {
            limit = text.length();
            digits = new StringBuilder();
            i = 0;
            while (i < limit && Character.isDigit(c = text.charAt(i))) {
                digits.append((char) c);
                i++;
            }
            return Integer.parseInt(digits.toString());
        } catch (Exception e) {
            return -1;
        }
    }

    static String later(String text) {
        int end;
        try {
            end = text.indexOf(';');
            if (end < 0) {
                throw new IllegalArgumentException("no end in " + text);
            }
            System.out.println("end " + end);
        } catch (Throwable t) {
            System.out.println("caught " + t.getMessage());
        }
        end = text.length();
        return text.substring(0, end / 2);
    }

    static int after(int[] values, int at) {
        int found;
        try {
            found = values[at];
        } catch (Exception e) {
            found = -2;
        }
        return found * 10;
    }

    static String rebuilt(String text, boolean twice) {
        StringBuilder s;
        s = new StringBuilder(text);
        try {
            s = new StringBuilder(s).append('!');
            if (twice) {
                s.append(text.charAt(10));
            }
        } catch (Exception e) {
            s = new StringBuilder("caught " + e.getClass().getSimpleName());
        }
        return s.toString();
    }

    static String chosen(String text) {
        StringBuilder filter;
        try {
            switch (text.charAt(0)) {
                case '&':
                    filter = new StringBuilder("and ");
                    filter.append(text.length());
                    break;
                case '|':
                    filter = new StringBuilder("or ");
                    filter.append(text.length());
                    break;
                default:
                    filter = new StringBuilder("atom ");
                    filter.append(text.charAt(1));
                    break;
            }
        } catch (Exception e) {
            return "caught " + e.getClass().getSimpleName();
        }
        return filter.toString();
    }

    public static void main(String[] args) {
        System.out.println(parse("123abc"));
        System.out.println(parse(""));
        System.out.println(parse(null));
        System.out.println(later("ab;cd"));
        System.out.println(later("abcd"));
        System.out.println(after(new int[] {4, 5}, 1));
        System.out.println(after(new int[] {4, 5}, 7));
        System.out.println(rebuilt("abc", false));
        System.out.println(rebuilt("abc", true));
        for (String text : new String[] {"&ab", "|c", "xy", "x", ""}) {
            System.out.println(chosen(text));
        }
    }
}
