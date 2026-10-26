import java.util.HashMap;
import java.util.Map;

// A switch on a boxed value, which Java unboxes, as the Minecraft server's
// `switch (state.getValue(PICKLES))` on the `Integer` of a generic property.
public class SwitchOnBoxes {
    static class Property<T extends Comparable<T>> {
        final T value;

        Property(T value) {
            this.value = value;
        }

        T get() {
            return value;
        }
    }

    static String shape(Property<Integer> pickles) {
        return switch (pickles.get()) {
            case 2 -> "two";
            case 3 -> "three";
            case 4 -> "four";
            default -> "one";
        };
    }

    static int age(Character c) {
        switch (c) {
            case 'a':
                return 1;
            case 'b':
                return 2;
            default:
                return 0;
        }
    }

    public static void main(String[] args) {
        for (int i = 1; i <= 5; i++) {
            System.out.println(shape(new Property<>(i)));
        }
        Map<String, Integer> ages = new HashMap<>();
        ages.put("x", 3);
        switch (ages.get("x")) {
            case 3 -> System.out.println("three");
            default -> System.out.println("other");
        }
        System.out.println(age('a') + age('b') + age('z'));
        try {
            switch (ages.get("missing")) {
                case 1 -> System.out.println("one");
                default -> System.out.println("other");
            }
        } catch (NullPointerException e) {
            System.out.println("null selector");
        }
    }
}
