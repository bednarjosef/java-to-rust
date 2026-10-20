// Builders and strings that may be null: a builder field set later, a static builder shared by
// methods, arrays of builders, null checks that narrow a builder, == on builders and null, and
// string methods on fields and on strings that may be null. The last line calls a method of a
// null string, which throws NullPointerException.
public class BuildersAndNulls {
    static StringBuilder log = new StringBuilder();

    static void note(String s) {
        log.append(s).append(';');
    }

    static class Document {
        String title;
        StringBuilder body;

        Document(String title) {
            this.title = title;
        }

        void write(String text) {
            if (body == null) {
                body = new StringBuilder();
            }
            body.append(text);
        }

        String render() {
            return title.toUpperCase() + ": " + (body == null ? "(empty)" : body.toString().trim());
        }
    }

    static int lengthOf(StringBuilder sb) {
        if (sb != null) {
            return sb.length();
        }
        return -1;
    }

    public static void main(String[] args) {
        note("start");
        note("next");
        System.out.println(log + " " + log.length());

        Document doc = new Document("notes");
        System.out.println(doc.render());
        doc.write("  first line ");
        doc.write("and more  ");
        System.out.println(doc.render() + " " + doc.title.length() + " " + doc.title.indexOf('t'));

        StringBuilder[] parts = new StringBuilder[3];
        parts[0] = new StringBuilder("zero");
        parts[2] = parts[0];
        parts[2].append("!");
        System.out.println(parts[0] + " " + parts[1] + " " + (parts[0] == parts[2]) + " " + (parts[1] == null));
        System.out.println(lengthOf(parts[0]) + " " + lengthOf(parts[1]));

        StringBuilder maybe = args.length > 3 ? new StringBuilder("x") : null;
        System.out.println("maybe: " + maybe + " " + (maybe == null) + " " + String.valueOf(maybe));
        StringBuilder target = new StringBuilder();
        target.append(maybe).append(" and ").append((String) null);
        System.out.println(target);

        String missing = args.length > 3 ? "here" : null;
        String present = args.length > 3 ? null : "Here";
        System.out.println(present.toLowerCase() + " " + present.equalsIgnoreCase(missing) + " " + present.contains("er"));
        System.out.println(String.join("/", present, missing, "end") + " " + String.format("%s|%S", missing, present));
        System.out.println(missing.length());
    }
}
