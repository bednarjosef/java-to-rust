// The methods of an enum whose fields are tables (a constructor that does more than set fields
// to constants), called inside a `try` that catches what they could throw, in a program that
// catches the errors of static initializers too, as javac's `Tokens.TokenKind.toString`: reading
// a field is reading a row of its table, which a constant always has, and the enum is
// initialized by the time one of its constants exists.

public class TableEnumMethodsCaught {
    enum Kind {
        IDENT("identifier", 1),
        DOT(".", 2),
        COMMA(",", 3),
        EOF(null, 0);

        final String name;
        final int weight;

        Kind(String name, int weight) {
            this.name = name;
            this.weight = weight * weight + (name == null ? 0 : name.length());
        }

        @Override
        public String toString() {
            switch (this) {
                case IDENT:
                    return "token." + name;
                case DOT:
                case COMMA:
                    return "'" + name + "'";
                default:
                    return name;
            }
        }

        int heavier(Kind other) {
            return weight - other.weight;
        }
    }

    static class Failing {
        static final int VALUE = Integer.parseInt("not a number");
    }

    public static void main(String[] args) {
        for (Kind k : Kind.values()) {
            try {
                System.out.println("kind " + k + " weighs " + k.weight + ", " + k.heavier(Kind.DOT) + " more than a dot");
            } catch (RuntimeException e) {
                System.out.println("failed: " + e);
            }
        }
        try {
            System.out.println(Failing.VALUE);
        } catch (ExceptionInInitializerError e) {
            System.out.println("initializer failed: " + e.getCause().getMessage());
        }
    }
}
