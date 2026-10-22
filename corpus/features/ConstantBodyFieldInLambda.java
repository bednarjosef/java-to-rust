// A field of an enum constant's body read in a lambda in the body's method: the enum's field
// of that constant.
import java.util.function.*;
public enum ConstantBodyFieldInLambda {
    PLAIN("a", "n"),
    LINT("b", "x") {
        private final String FORMAT = "%s=" + "%s";
        @Override
        public void process(String option) {
            System.out.println(String.format(FORMAT, option, 1));
            Consumer<String> c = key -> System.out.println(String.format(FORMAT, key, 2));
            c.accept("k");
        }
    },
    INHERIT("c", "m") {
        @Override
        public void process(String option) {
            System.out.println(helper(option));
        }
        private String helper(String s) { return s + "!"; }
    };

    final String text;
    final String arg;
    ConstantBodyFieldInLambda(String text, String arg) { this.text = text; this.arg = arg; }

    public void process(String option) { System.out.println(text + option); }

    public static void main(String[] args) {
        for (ConstantBodyFieldInLambda o : values()) o.process("z");
    }
}
