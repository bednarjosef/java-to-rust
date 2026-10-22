// Objects that a thread reaches are shown as text by their toString, which reads their fields
// through the object's lock: the text of a shared object inside a concatenation, of one that is
// a field of another shared object, and of one that is a method's argument.
public class SharedObjectsInText {
    static class Span {
        int start;
        int end;

        Span(int start, int end) {
            this.start = start;
            this.end = end;
        }

        void shift(int by) {
            start += by;
            end += by;
        }

        @Override
        public String toString() {
            return "Span[start=" + start + ", end=" + end + "]";
        }
    }

    static class Range {
        Span span;
        String lint;

        Range(Span span, String lint) {
            this.span = span;
            this.lint = lint;
        }

        @Override
        public String toString() {
            return "Range[span=" + span + ", lint=" + lint + "]";
        }
    }

    static class Labelled {
        String name;
        Span span;

        Labelled(String name, Span span) {
            this.name = name;
            this.span = span;
        }

        void rename(String to) {
            name = to;
        }

        String describe(Span other) {
            return name + " at " + span + " and " + other;
        }
    }

    static class Tag {
        String name;
        int count;

        Tag(String name, int count) {
            this.name = name;
            this.count = count;
        }

        @Override
        public String toString() {
            return "Tag(" + name + ", " + count + ")";
        }

        void bump() {
            count++;
        }
    }

    static Range shared;
    static Labelled labelled;
    static Tag tag;

    static class Printer extends Thread {
        @Override
        public void run() {
            System.out.println("shared: " + shared);
            System.out.println("labelled: " + labelled.describe(shared.span));
            labelled.span.shift(10);
            System.out.println("moved: " + labelled.span + " in " + shared);
            System.out.println("tag: " + tag + " and " + labelled.span);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Span span = new Span(1, 4);
        shared = new Range(span, "unused");
        labelled = new Labelled("first", new Span(2, 3));
        Printer printer = new Printer();
        tag = new Tag("t", 2);
        printer.start();
        printer.join();
        tag.bump();
        System.out.println("tag bumped: " + tag);
        tag = null;
        System.out.println("tag after: " + tag);
        labelled.rename("second");
        span.shift(-1);
        System.out.println(shared + " / " + labelled.describe(span));
    }
}
