import java.util.ArrayList;
import java.util.List;

/**
 * An enum's constants with bodies that override the enum's methods and call them with `super`,
 * as Guava's `LocalCache.EntryFactory` copies an entry: `super` runs the enum's own method, not
 * the constant's again. The enum implements an interface's method too, as `IsoFields.Field`'s
 * `rangeRefinedBy` implements `TemporalField`'s.
 */
public class EnumConstantSuperCalls {
    interface Measure {
        int size(String text);
    }

    enum Factory implements Measure {
        PLAIN,
        LOUD {
            @Override
            String make(String text) {
                return super.make(text).toUpperCase();
            }
        },
        TWICE {
            @Override
            String make(String text) {
                String made = super.make(text);
                return made + "+" + super.make(made);
            }

            @Override
            <T> List<T> copy(List<T> items, T extra) {
                List<T> copied = super.copy(items, extra);
                copied.add(extra);
                return copied;
            }

            @Override
            public int size(String text) {
                return 2 * super.size(text);
            }
        };

        @Override
        public int size(String text) {
            return text.length();
        }

        String make(String text) {
            return "made " + text + " by " + name();
        }

        <T> List<T> copy(List<T> items, T extra) {
            List<T> copied = new ArrayList<>(items);
            copied.add(extra);
            return copied;
        }
    }

    public static void main(String[] args) {
        for (Factory factory : Factory.values()) {
            System.out.println(factory.make("x"));
            System.out.println(factory.copy(List.of(1, 2), 3));
            Measure measure = factory;
            System.out.println(measure.size("abc") + " " + factory.size("abcd"));
        }
    }
}
