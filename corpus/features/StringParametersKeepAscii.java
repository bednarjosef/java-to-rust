import java.util.function.Consumer;

// A string parameter that a method only reads, and reads by code unit index (charAt, indexOf,
// substring, length), is given with whether its text is ASCII, which the caller's string keeps:
// literals, strings built at run time, other such parameters passed on, temporaries, fields,
// statics and array elements, ASCII or not, with characters outside the BMP and lone surrogates.
public class StringParametersKeepAscii {
    static String banner = "static text";

    interface Measure {
        int measure(String s, int i);
    }

    static class Units implements Measure {
        public int measure(String s, int i) {
            return s.charAt(i) + s.indexOf('a', i);
        }
    }

    static class Tail implements Measure {
        public int measure(String s, int i) {
            return s.substring(i).length() * 1000 + s.lastIndexOf(' ');
        }
    }

    final String name;

    StringParametersKeepAscii(String name) {
        this.name = name;
    }

    static int nextSpace(String s, int from) {
        return s.indexOf(' ', from);
    }

    static String word(String s, int from, int to) {
        return s.substring(from, to);
    }

    static int unitAt(String s, int i) {
        return s.charAt(i);
    }

    static String words(String s) {
        StringBuilder out = new StringBuilder();
        int from = 0;
        for (int i = nextSpace(s, 0); i >= 0; i = nextSpace(s, from)) {
            out.append('[').append(word(s, from, i)).append(':').append(unitAt(s, from)).append(']');
            from = i + 1;
        }
        return out.append(s.substring(from)).append(' ').append(s.length()).toString();
    }

    static int depth(String s, int i) {
        if (i >= s.length()) {
            return 0;
        }
        return s.charAt(i) % 7 + depth(s, i + 1);
    }

    static String safely(String s, int i) {
        try {
            return "" + s.charAt(i) + s.substring(i, i + 2);
        } catch (StringIndexOutOfBoundsException e) {
            return "out of bounds: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(words("plain ascii words"));
        System.out.println(words("čaj a káva x😀y konec"));
        System.out.println(words("lone \ud800 surrogate \udc00 here"));
        StringBuilder built = new StringBuilder();
        for (int i = 0; i < 5; i++) {
            built.append("w").append(i).append(i % 2 == 0 ? "é " : " ");
        }
        String text = built.toString();
        System.out.println(words(text));
        System.out.println(words(text.substring(4)));
        System.out.println(words(text + "end word"));
        System.out.println(words(banner));
        System.out.println(words(new StringParametersKeepAscii("field value x").name));
        String[] parts = {"array one", "dva 😀 tři"};
        for (String p : parts) {
            System.out.println(words(p) + " " + depth(p, 0));
        }
        String maybe = args.length > 5 ? null : "narrowed text";
        if (maybe != null) {
            System.out.println(words(maybe));
        }
        Measure[] measures = {new Units(), new Tail()};
        for (Measure m : measures) {
            System.out.println(m.measure("a ab abc", 2) + " " + m.measure("é á a", 1));
        }
        Consumer<String> show = s -> System.out.println(words(s) + " " + unitAt(s, 0));
        show.accept("from a lambda");
        show.accept("😀 lambda");
        System.out.println(safely("ab", 1));
        System.out.println(safely("é", 0));
    }
}
