import java.util.Objects;

// String hash codes, which a string keeps once computed: of fields that may be null, of strings
// built piece by piece, of text that is not ASCII, and of strings whose hash is 0 (the empty
// string and "f5a5a608"), asked for again and again.
public class StringHashesKept {
    static final class Named {
        final String name;
        String alias;

        Named(String name) {
            this.name = name;
        }

        @Override
        public int hashCode() {
            return 31 * name.hashCode() + Objects.hashCode(alias);
        }
    }

    static String built(int n) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < n; i++) {
            b.append((char) ('a' + i % 26));
        }
        return b.toString();
    }

    public static void main(String[] args) {
        String[] texts = {"", "f5a5a608", "minecraft", "naïve", "😀 smile", "\uD800 alone", built(40), built(3)};
        for (String s : texts) {
            int first = s.hashCode();
            int again = s.hashCode();
            System.out.println(first + " " + (first == again) + " " + (s.length() > 0 ? s.charAt(0) : '-'));
        }
        Named[] named = new Named[texts.length];
        for (int i = 0; i < texts.length; i++) {
            named[i] = new Named(texts[i]);
            if (i % 2 == 0) {
                named[i].alias = texts[(i + 1) % texts.length] + i;
            }
        }
        long sum = 0;
        for (int round = 0; round < 1000; round++) {
            for (Named n : named) {
                sum += n.hashCode();
            }
            named[round % named.length].alias = round % 3 == 0 ? null : "alias " + round;
        }
        System.out.println("sum " + sum);
        String grown = "";
        for (int i = 0; i < 5; i++) {
            grown = grown + i;
            System.out.println(grown + " " + grown.hashCode());
        }
        System.out.println(Objects.hashCode(null) + " " + Objects.hashCode(named[1].alias) + " " + Objects.hash("a", "b"));
    }
}
