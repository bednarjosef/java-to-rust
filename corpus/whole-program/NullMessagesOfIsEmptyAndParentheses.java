import java.util.ArrayList;
import java.util.List;

// Null messages of calls that the translation writes differently from javac: `isEmpty()` of a
// `CharSequence`, which it writes as a zero `length()`, and the calls of a for-each loop over a
// value in parentheses, which javac starts at the parenthesis.
public class NullMessagesOfIsEmptyAndParentheses {
    static class Text implements CharSequence {
        final String s;

        Text(String s) {
            this.s = s;
        }

        public int length() {
            return s.length();
        }

        public char charAt(int i) {
            return s.charAt(i);
        }

        public CharSequence subSequence(int a, int b) {
            return s.subSequence(a, b);
        }

        public String toString() {
            return s;
        }
    }

    static class Element {
        final Text name;
        final List<Element> enclosed;

        Element(Text name, List<Element> enclosed) {
            this.name = name;
            this.enclosed = enclosed;
        }

        Text getSimpleName() {
            return name;
        }

        List<Element> getEnclosedElements() {
            return enclosed;
        }
    }

    static boolean unnamed(Element e) {
        return e.getSimpleName().isEmpty();
    }

    static int count(Element e, boolean all) {
        int n = 0;
        for (Element element : (all ? e.getEnclosedElements() : e.getEnclosedElements().subList(0, 1))) {
            n += unnamed(element) ? 0 : 1;
        }
        return n;
    }

    static void attempt(String label, java.util.function.Supplier<Object> s) {
        try {
            System.out.println(label + ": " + s.get());
        } catch (NullPointerException e) {
            System.out.println(label + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        List<Element> children = new ArrayList<>();
        children.add(new Element(new Text("a"), null));
        children.add(new Element(new Text(""), null));
        Element top = new Element(new Text("top"), children);
        attempt("unnamed", () -> unnamed(top));
        attempt("null name", () -> unnamed(new Element(null, null)));
        attempt("count all", () -> count(top, true));
        attempt("count first", () -> count(top, false));
        attempt("no children", () -> count(new Element(new Text("x"), null), true));
        children.add(new Element(null, null));
        attempt("null child name", () -> count(top, true));
    }
}
