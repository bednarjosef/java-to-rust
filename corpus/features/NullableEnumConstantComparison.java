public class NullableEnumConstantComparison {
    enum Kind { COMMA, DOT, END }

    static class Token {
        Kind kind;

        Token(Kind kind) {
            this.kind = kind;
        }
    }

    static Kind lastSeen;

    static Kind pick(int n) {
        if (n < 0) {
            return null;
        }
        if (n == 2) {
            return Kind.END;
        }
        return n == 0 ? Kind.COMMA : Kind.DOT;
    }

    static boolean isComma(Kind k) {
        return k == Kind.COMMA;
    }

    static String describe(Kind k) {
        return k == null ? "none" : k.name();
    }

    static int countCommas(Token token) {
        int count = 0;
        do {
            count++;
            token.kind = pick(count);
        } while (token.kind == Kind.COMMA || token.kind == null);
        return count;
    }

    static int countNotEnd(Token token) {
        int count = 0;
        do {
            count++;
            token.kind = pick(count);
        } while (token.kind != Kind.END);
        return count;
    }

    public static void main(String[] args) {
        System.out.println(countCommas(new Token(Kind.COMMA)));
        System.out.println(countCommas(new Token(null)));
        System.out.println(countCommas(new Token(Kind.END)));
        System.out.println(countNotEnd(new Token(Kind.COMMA)));
        System.out.println(countNotEnd(new Token(null)));

        System.out.println(isComma(Kind.COMMA) + " " + isComma(null) + " " + isComma(Kind.END));

        lastSeen = Kind.DOT;
        System.out.println(describe(lastSeen));
        lastSeen = null;
        System.out.println(describe(lastSeen));

        Kind k = pick(-1);
        if (k != Kind.END) {
            System.out.println("not end: " + describe(k));
        }
        k = pick(0);
        if (k != Kind.END) {
            System.out.println("not end: " + describe(k));
        }

        Token missing = new Token(null);
        try {
            System.out.println(missing.kind.name());
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
