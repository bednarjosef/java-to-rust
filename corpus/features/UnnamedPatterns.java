// Unnamed pattern variables (Java 22): `case A _, B _` shares one case among several type
// patterns, and `_` alone in a record pattern matches any component.
public class UnnamedPatterns {
    sealed interface Value permits Text, Number, Pair, Flag, Nothing {}
    record Text(String s) implements Value {}
    record Number(int n) implements Value {}
    record Pair(Value left, Value right) implements Value {}
    record Flag(boolean on) implements Value {}
    record Nothing() implements Value {}

    static int size(Value v) {
        return 1 + switch (v) {
            case Pair p -> size(p.left()) + size(p.right());
            case Text _, Number _ -> 2;
            case Flag _ -> 4;
            case Nothing _ -> 0;
        };
    }

    static String describe(Object o) {
        return switch (o) {
            case Pair(Text t, _) -> "pair starting with text " + t.s();
            case Pair(_, Number(int n)) -> "pair ending with number " + n;
            case Pair _ -> "another pair";
            case String _, Integer _ -> "a string or an integer: " + o;
            case null -> "null";
            default -> "something else";
        };
    }

    public static void main(String[] args) {
        Value[] values = {
            new Text("a"), new Number(3), new Flag(true), new Nothing(),
            new Pair(new Text("x"), new Pair(new Number(1), new Flag(false))),
        };
        for (Value v : values) {
            System.out.println(v + " -> " + size(v));
        }
        Object[] objects = {
            new Pair(new Text("t"), new Nothing()), new Pair(new Flag(true), new Number(7)),
            new Pair(new Nothing(), new Nothing()), "s", 42, null, 2.5,
        };
        for (Object o : objects) {
            System.out.println(describe(o));
        }
        if (values[1] instanceof Number _) {
            System.out.println("a number, unnamed");
        }
    }
}
