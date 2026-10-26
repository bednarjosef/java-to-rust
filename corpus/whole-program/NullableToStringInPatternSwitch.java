// A pattern switch's default arm that calls a method which may return null (`Tag.toString()`,
// which `Odd` answers with null), in a method that may return null: the arm's value is the
// call's own, not wrapped again (Minecraft's `MacroFunction.stringify`).
public class NullableToStringInPatternSwitch {
    interface Tag {
        String toString();
    }

    record IntTag(int v) implements Tag {}

    record StrTag(String s) implements Tag {}

    static final class Odd implements Tag {
        public String toString() {
            return null;
        }
    }

    static String stringify(Tag tag) {
        return switch (tag) {
            case IntTag(int v) -> String.valueOf(v);
            case StrTag(String s) -> s;
            default -> tag.toString();
        };
    }

    public static void main(String[] args) {
        String odd = stringify(new Odd());
        System.out.println((odd == null) + " " + (stringify(new StrTag(null)) == null));
        System.out.println(stringify(new IntTag(3)) + " " + stringify(new Odd()) + " " + (stringify(new Tag() {
            public String toString() {
                return "anon";
            }
        })));
    }
}
