import java.util.EnumSet;
import java.util.List;
import java.util.Objects;
import java.util.function.BiPredicate;
import java.util.function.Function;
import java.util.stream.Collectors;

// `Object`'s methods named on an enum's constants: `Object::toString` over a stream of them, as
// the Minecraft server's `VanillaAdventureAdvancements.validateMobsToKill` writes it, which calls
// the enum's own `toString`, a constant body's too.
public class EnumsThroughObjectMethods {
    interface Named {
        String serializedName();
    }

    enum Category implements Named {
        MONSTER,
        CREATURE,
        AMBIENT;

        public String serializedName() {
            return name().toLowerCase();
        }
    }

    enum Shape {
        ROUND {
            @Override
            public String toString() {
                return "a round shape";
            }
        },
        SQUARE;

        @Override
        public String toString() {
            return "shape " + name();
        }
    }

    public static void main(String[] args) {
        System.out.println(EnumSet.allOf(Category.class).stream()
            .map(Object::toString).sorted().collect(Collectors.joining(", ")));
        System.out.println(List.of(Shape.SQUARE, Shape.ROUND).stream()
            .map(Object::toString).collect(Collectors.joining("; ")));
        Function<Shape, String> text = Object::toString;
        System.out.println(text.apply(Shape.ROUND) + " / " + text.apply(Shape.SQUARE));
        BiPredicate<Category, Object> same = Object::equals;
        System.out.println(same.test(Category.AMBIENT, Category.AMBIENT) + " " + same.test(Category.AMBIENT, Category.MONSTER)
            + " " + same.test(Category.AMBIENT, "AMBIENT"));
        Function<Category, Integer> hash = Object::hashCode;
        System.out.println(hash.apply(Category.CREATURE).intValue() == Category.CREATURE.hashCode());
        System.out.println(Objects.equals(hash.apply(Category.MONSTER), hash.apply(Category.MONSTER)));
        try {
            text.apply(null);
        } catch (NullPointerException e) {
            System.out.println("null shape");
        }
    }
}
