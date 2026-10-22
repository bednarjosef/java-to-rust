// An anonymous class in an instance method, whose superclass constructor's parameter is named
// `outer`, as javac's ClassReader makes `new ClassType(outer, actuals, t) { .. }`.
public class AnonymousSuperParameterNames {
    static class Type {
        final Type outer;
        final String name;

        Type(Type outer, String name) {
            this.outer = outer;
            this.name = name;
        }

        String show() {
            return (outer == null ? "" : outer.show() + ".") + name;
        }
    }

    String prefix = "reader";

    Type read(Type outer, String name) {
        return new Type(outer, name) {
            String show() {
                return prefix + ":" + super.show();
            }
        };
    }

    public static void main(String[] args) {
        AnonymousSuperParameterNames reader = new AnonymousSuperParameterNames();
        Type t = reader.read(reader.read(null, "Outer"), "Inner");
        System.out.println(t.show());
    }
}
