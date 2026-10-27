import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.function.Predicate;

// An enum whose constants' bodies override `toString()`, which the enum itself does not, as
// Guava's `Predicates.ObjectPredicate` does: each way of asking for a constant's text runs the
// override, and a constant without one answers its name.
public class EnumBodyToString {
    enum ObjectPredicate implements Predicate<Object> {
        ALWAYS_TRUE {
            @Override
            public boolean test(Object o) {
                return true;
            }

            @Override
            public String toString() {
                return "Predicates.alwaysTrue()";
            }
        },
        IS_NULL {
            @Override
            public boolean test(Object o) {
                return o == null;
            }

            @Override
            public String toString() {
                return "Predicates.isNull()";
            }
        },
        NOT_NULL {
            @Override
            public boolean test(Object o) {
                return o != null;
            }
        };

        @SuppressWarnings("unchecked")
        <T> Predicate<T> narrowed() {
            return (Predicate<T>) this;
        }
    }

    public static void main(String[] args) {
        for (ObjectPredicate p : ObjectPredicate.values()) {
            Object o = p;
            Predicate<String> narrowed = p.narrowed();
            System.out.println(p + " | " + p.toString() + " | " + o + " | " + String.valueOf(o) + " | "
                    + narrowed + " | " + p.name() + " " + p.test(null));
        }
        System.out.println(EnumSet.allOf(ObjectPredicate.class));
        System.out.println(Arrays.toString(ObjectPredicate.values()));
        List<Predicate<Object>> all = List.of(ObjectPredicate.IS_NULL, ObjectPredicate.NOT_NULL);
        System.out.println(all + " " + ObjectPredicate.valueOf("IS_NULL"));
        StringBuilder text = new StringBuilder().append(ObjectPredicate.ALWAYS_TRUE).append(' ');
        text.append((Object) ObjectPredicate.NOT_NULL);
        System.out.println(text);
    }
}
