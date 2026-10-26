// A class built whole (its constructor calls a method that a subclass overrides) whose
// constructor's parameter is named obj, as the factory's local for the new object is
// (xpath's XObject(Object obj)).
public class WholeObjectParameterNamedObj {
    static class Value {
        protected Object obj;

        Value(Object obj) {
            this.obj = obj;
            System.out.println("made " + describe());
        }

        String describe() {
            return "value " + obj;
        }
    }

    static class Number extends Value {
        Number(double d) {
            super(d);
        }

        String describe() {
            return "number " + obj;
        }
    }

    public static void main(String[] args) {
        Value[] values = {new Value("text"), new Number(2.5), new Value(null)};
        for (Value v : values) {
            System.out.println(v.describe());
        }
    }
}
