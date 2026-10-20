// Features: StringBuilder with append, insert, reverse, setCharAt, deleteCharAt, setLength and
// indexOf, String.format and printf with width, precision, left and right alignment, zero padding,
// hex and grouped integers, String methods repeat, strip, chars, toCharArray, compareTo,
// String.valueOf, text blocks, a text-wrapping routine, a box-drawing table renderer, and char
// arithmetic for a Caesar cipher.
import java.util.ArrayList;
import java.util.List;

public class StringBuilderTable {
    record Row(String item, int qty, double price) {
    }

    static String pad(String s, int width, boolean left) {
        if (s.length() >= width) {
            return s;
        }
        String spaces = " ".repeat(width - s.length());
        return left ? s + spaces : spaces + s;
    }

    static List<String> wrap(String text, int width) {
        List<String> lines = new ArrayList<>();
        StringBuilder line = new StringBuilder();
        for (String word : text.split("\\s+")) {
            if (line.length() > 0 && line.length() + 1 + word.length() > width) {
                lines.add(line.toString());
                line.setLength(0);
            }
            if (line.length() > 0) {
                line.append(' ');
            }
            line.append(word);
        }
        if (line.length() > 0) {
            lines.add(line.toString());
        }
        return lines;
    }

    static String caesar(String text, int shift) {
        StringBuilder sb = new StringBuilder(text.length());
        for (char c : text.toCharArray()) {
            if (Character.isUpperCase(c)) {
                sb.append((char) ('A' + (c - 'A' + shift + 26) % 26));
            } else if (Character.isLowerCase(c)) {
                sb.append((char) ('a' + (c - 'a' + shift + 26) % 26));
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    static String box(List<Row> rows) {
        int itemWidth = "Item".length();
        for (Row r : rows) {
            itemWidth = Math.max(itemWidth, r.item().length());
        }
        String rule = "+" + "-".repeat(itemWidth + 2) + "+" + "-".repeat(7) + "+" + "-".repeat(11) + "+";
        StringBuilder out = new StringBuilder();
        out.append(rule).append('\n');
        out.append(String.format("| %-" + itemWidth + "s | %5s | %9s |%n", "Item", "Qty", "Price"));
        out.append(rule).append('\n');
        double grand = 0;
        for (Row r : rows) {
            out.append(String.format("| %-" + itemWidth + "s | %5d | %9.2f |%n", r.item(), r.qty(), r.price()));
            grand += r.qty() * r.price();
        }
        out.append(rule).append('\n');
        out.append(String.format("| %-" + itemWidth + "s | %5s | %9.2f |%n", "total", "", grand));
        out.append(rule);
        return out.toString();
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder("hello");
        sb.insert(0, ">> ").append(" world").reverse();
        System.out.println(sb + " length " + sb.length());
        sb.reverse();
        sb.setCharAt(3, 'H');
        sb.deleteCharAt(sb.length() - 1);
        System.out.println(sb + " index of o: " + sb.indexOf("o") + " last: " + sb.lastIndexOf("o"));
        sb.setLength(5);
        System.out.println("truncated: [" + sb + "]");

        List<Row> rows = List.of(new Row("Pencil", 12, 0.5), new Row("Notebook", 3, 4.25),
                new Row("Stapler", 1, 12.999), new Row("Eraser", 25, 0.2));
        System.out.println(box(rows));

        System.out.println(pad("left", 8, true) + "|" + pad("right", 8, false) + "|");
        System.out.printf("[%8.3f] [%-8.1f] [%08d] [%+d] [%x] [%X] [%o]%n", Math.PI, 2.25, 42, 7, 255, 255, 8);
        System.out.printf("[%,d] [%10s] [%-10s] [%c] [%b] [%%]%n", 1234567, "right", "left", 'Z', true);
        System.out.println(String.format("%5.1e", 12345.678));
        System.out.println(String.valueOf(3.0) + " " + String.valueOf(new char[] {'o', 'k'}) + " " + "ab".compareTo("ac"));
        System.out.println("  padded  ".strip() + "|" + "=".repeat(10) + "|" + "Mississippi".chars().filter(c -> c == 's').count());

        String paragraph = "Java programs often build strings in loops and then print them, which is why "
                + "StringBuilder exists and why wrapping text is a common small task.";
        for (String line : wrap(paragraph, 28)) {
            System.out.println("|" + pad(line, 28, true) + "|");
        }

        String secret = caesar("Attack at Dawn, Zed!", 3);
        System.out.println(secret + " -> " + caesar(secret, -3));

        String block = """
                first line
                  indented "quoted"
                last\
                 joined
                """;
        System.out.print(block);
        System.out.println(block.lines().count() + " lines, " + block.length() + " chars");
    }
}
