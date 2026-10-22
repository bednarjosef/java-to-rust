// A `?:` between a nullable enum and a constant of the same enum, as javac's `Tokens.lookupKind`
// writes it: `t != null ? t : TokenKind.IDENTIFIER`, where `t` comes from a lookup that may find
// no entry. The result is never null, so it is the enum itself; the nullable value is only read
// where the test has shown it is not null.
public class NullableEnumConditional {
    enum Kind { IDENTIFIER, KEYWORD, COMMA }

    static class Tokens {
        final String[] names = {"if", ","};
        final Kind[] kinds = {Kind.KEYWORD, Kind.COMMA};

        Kind find(String name) {
            for (int i = 0; i < names.length; i++) {
                if (names[i].equals(name)) {
                    return kinds[i];
                }
            }
            return null;
        }

        Kind lookupKind(String name) {
            Kind t = find(name);
            return t != null ? t : Kind.IDENTIFIER;
        }

        Kind lookupKind(String name, Kind fallback) {
            Kind t = find(name);
            return (t != null) ? t : fallback;
        }
    }

    static Kind orDefault(Kind maybe) {
        return maybe != null ? maybe : Kind.COMMA;
    }

    public static void main(String[] args) {
        Tokens tokens = new Tokens();
        System.out.println(tokens.lookupKind("if"));
        System.out.println(tokens.lookupKind("while"));
        System.out.println(tokens.lookupKind(","));
        System.out.println(tokens.lookupKind("else", Kind.KEYWORD));
        System.out.println(tokens.lookupKind("if", Kind.IDENTIFIER));
        System.out.println(orDefault(null) + " " + orDefault(Kind.KEYWORD));
        Kind last = tokens.lookupKind("x");
        System.out.println(last == Kind.IDENTIFIER);
    }
}
