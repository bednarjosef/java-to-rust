import java.lang.constant.Constable;
import java.lang.constant.ConstantDesc;
import java.util.Optional;

// `describeConstable()` and `resolveConstantDesc(lookup)` of strings and boxes held as a
// `Constable` or a `ConstantDesc`, beside the program's own: a string, an `Integer`, a `Long`, a
// `Float` and a `Double` describe themselves, and resolve to themselves.
public class ConstablesOfStringsAndBoxes {
    record Point(int x) implements Constable {
        @Override
        public Optional<? extends ConstantDesc> describeConstable() {
            return Optional.of(x * 10);
        }
    }

    static String describe(Constable c) {
        Optional<? extends ConstantDesc> d = c.describeConstable();
        return d.isPresent() ? "described " + d.get() : "not described";
    }

    static String resolve(ConstantDesc d) throws ReflectiveOperationException {
        Object resolved = d.resolveConstantDesc(null);
        return "resolved " + resolved + " " + (resolved == d);
    }

    public static void main(String[] args) throws Exception {
        Constable[] constables = {"text", 42, 7L, 1.5f, 2.25, new Point(3)};
        for (Constable c : constables) {
            System.out.println(describe(c));
        }
        ConstantDesc[] descs = {"desc", 5, 9L, 0.5f, -3.0};
        for (ConstantDesc d : descs) {
            System.out.println(resolve(d));
        }
        Constable same = "same";
        System.out.println(same.describeConstable().get() == same);
        String text = "direct";
        Optional<String> described = text.describeConstable();
        System.out.println(described.get() + " " + (described.get() == text));
        Integer boxed = 1000;
        System.out.println(boxed.describeConstable().get() + " " + (boxed.resolveConstantDesc(null) == boxed));
        Double d = 0.25;
        System.out.println(d.describeConstable().get() + " " + "x".resolveConstantDesc(null));
    }
}
