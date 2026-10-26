// Pattern switch cases whose last statement is a `try` or an `if` that leaves on every path, as
// the JDK's `RSAKeyFactory.generatePublic` yields from a `try` and throws from its `catch`.
public class PatternCasesLeavingInBranches {
    static int parse(String s) {
        return Integer.parseInt(s);
    }

    static String describe(Object o) {
        return switch (o) {
            case String s -> {
                try {
                    yield "number " + parse(s);
                } catch (NumberFormatException e) {
                    throw new IllegalArgumentException("not a number: " + s);
                }
            }
            case Integer i -> {
                if (i < 0) {
                    yield "negative";
                } else {
                    yield "count " + i;
                }
            }
            case null -> "nothing";
            default -> {
                try {
                    yield o.getClass().getSimpleName();
                } finally {
                    System.out.println("described");
                }
            }
        };
    }

    static void print(Object o) {
        switch (o) {
            case String s:
                try {
                    System.out.println("text " + parse(s));
                    return;
                } catch (NumberFormatException e) {
                    System.out.println("bad text " + s);
                    return;
                }
            case Integer i:
                if (i > 10) {
                    System.out.println("big");
                    break;
                } else {
                    System.out.println("small");
                    break;
                }
            default:
                System.out.println("other");
        }
    }

    public static void main(String[] args) {
        System.out.println(describe("12"));
        try {
            describe("x");
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(describe(-3));
        System.out.println(describe(4));
        System.out.println(describe(null));
        System.out.println(describe(2.5));
        print("7");
        print("y");
        print(11);
        print(1);
        print(1.0);
    }
}
