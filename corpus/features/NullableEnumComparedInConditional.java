// A comparison of an enum that may be null, as one branch of a `?:`, as javac's
// `Modules.setupAllModules` writes `observable == null ? (finder.findModule(sym).kind != ERR) :
// observable.contains(sym)`. The enum operand here is an assignment, whose Rust starts with a
// block; at the start of the branch that block would end the expression, so it is in parentheses.
public class NullableEnumComparedInConditional {
    enum Kind { IDENTIFIER, KEYWORD, COMMA }

    static Kind last;

    static Kind find(String name) {
        if (name.equals("if")) {
            return Kind.KEYWORD;
        }
        return name.equals(",") ? Kind.COMMA : null;
    }

    static boolean notComma(boolean look, String name) {
        return look ? (last = find(name)) != Kind.COMMA : name.isEmpty();
    }

    public static void main(String[] args) {
        System.out.println(notComma(true, "if") + " " + last);
        System.out.println(notComma(true, ",") + " " + last);
        System.out.println(notComma(true, "x") + " " + last);
        System.out.println(notComma(false, ""));
    }
}
