// What a `toString` that may return null gives, passed to a method of an object whose fields
// change, as javac's `Attribute.Constant.accept` passes `toString()` to `visitString`. The program
// has no exceptions, so the text is an `Option` without a `Result` around it.
public class NullableTextAsArgument {
    abstract static class Value {
        abstract String show();

        public String toString() {
            return show();
        }
    }

    static class Missing extends Value {
        String show() {
            return null;
        }
    }

    static class Named extends Value {
        final String name;

        Named(String name) {
            this.name = name;
        }

        String show() {
            return name;
        }
    }

    static class Visitor {
        int visits;

        String visitString(String s, Object p) {
            visits++;
            return visits + ":" + s + "/" + p;
        }
    }

    public static void main(String[] args) {
        Value[] values = {new Named("first"), new Missing(), new Named("last")};
        Visitor visitor = new Visitor();
        Visitor same = visitor;
        for (Value v : values) {
            System.out.println(visitor.visitString(v.toString(), "p"));
        }
        System.out.println(same.visits);
    }
}
