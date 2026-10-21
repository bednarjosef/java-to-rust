public class Tokenizer {
    interface Token {
        String text();
        default boolean isSymbol(char c) { return false; }
        default int value() { return 0; }
    }

    static class Word implements Token {
        final String w;
        Word(String w) { this.w = w; }
        public String text() { return "word(" + w + ")"; }
    }

    static class Number implements Token {
        final int n;
        Number(int n) { this.n = n; }
        public String text() { return "num(" + n + ")"; }
        public int value() { return n; }
    }

    static class Symbol implements Token {
        final char c;
        Symbol(char c) { this.c = c; }
        public String text() { return "sym(" + c + ")"; }
        public boolean isSymbol(char x) { return c == x; }
    }

    static class Scanner {
        final String src;
        int pos;
        Token last;
        int count;
        Scanner(String src) { this.src = src; }

        boolean done() {
            while (pos < src.length() && src.charAt(pos) == ' ') pos++;
            return pos >= src.length();
        }

        Token next() {
            char c = src.charAt(pos);
            Token t;
            if (Character.isDigit(c)) {
                int v = 0;
                while (pos < src.length() && Character.isDigit(src.charAt(pos))) {
                    v = v * 10 + (src.charAt(pos) - '0');
                    pos++;
                }
                t = new Number(v);
            } else if (Character.isLetter(c)) {
                int start = pos;
                while (pos < src.length() && Character.isLetter(src.charAt(pos))) pos++;
                t = new Word(src.substring(start, pos));
            } else {
                pos++;
                t = new Symbol(c);
            }
            last = t;
            count++;
            return t;
        }
    }

    static int evaluate(Token[] tokens, int n) {
        int sum = 0;
        int sign = 1;
        for (int i = 0; i < n; i++) {
            Token t = tokens[i];
            if (t.isSymbol('-')) sign = -1;
            else if (t.isSymbol('+')) sign = 1;
            else { sum += sign * t.value(); }
        }
        return sum;
    }

    public static void main(String[] args) {
        Scanner s = new Scanner("12 + x - 30 + 7 end");
        Token[] tokens = new Token[20];
        int n = 0;
        StringBuilder out = new StringBuilder();
        while (!s.done()) {
            Token t = s.next();
            tokens[n++] = t;
            out.append(t.text()).append(' ');
        }
        System.out.println(out.toString().trim());
        System.out.println(n + " tokens, last " + s.last.text() + ", value " + evaluate(tokens, n));
        System.out.println(s.count + " " + (tokens[0] == s.last) + " " + (tokens[n - 1] == s.last));
        Token none = tokens[n];
        System.out.println(none == null);
    }
}
