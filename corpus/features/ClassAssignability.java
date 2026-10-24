// `Class.isAssignableFrom` of class values that are not class literals: a registry that accepts
// only the classes below its base class, as a multimap keyed by class does, a filter by a class
// parameter, and the rules for interfaces, enums, boxes, arrays and primitive types; and
// `asSubclass` of a class parameter, as a loader of plugins by a supertype that it is given does.
public class ClassAssignability {
    interface Shape {}

    interface Named {
        String name();
    }

    static class Circle implements Shape {}

    static class Ring extends Circle implements Named {
        public String name() {
            return "ring";
        }
    }

    record Square(String name) implements Shape, Named {}

    enum Suit implements Named {
        HEARTS,
        SPADES {
            @Override
            int weight() {
                return 2;
            }
        };

        int weight() {
            return 1;
        }
    }

    static final class Registry {
        final Class<?> base;

        Registry(Class<?> base) {
            this.base = base;
        }

        String accept(Class<?> c) {
            if (!base.isAssignableFrom(c)) {
                return "no " + c.getSimpleName();
            }
            return "yes " + c.getSimpleName();
        }
    }

    static int ofType(Object[] values, Class<?> type) {
        int count = 0;
        for (Object v : values) {
            if (type.isAssignableFrom(v.getClass())) {
                count++;
            }
        }
        return count;
    }

    static Class<?> pick(int i) {
        Class<?>[] classes = {
            Shape.class, Circle.class, Ring.class, Square.class, Named.class, Suit.class, Object.class,
            Number.class, Integer.class, String.class, CharSequence.class, Comparable.class, int.class,
            int[].class, long[].class, Shape[].class, Circle[].class, Object[].class, Enum.class
        };
        return classes[i];
    }

    static final StringBuilder order = new StringBuilder();

    static <C> Class<C> traced(String step, Class<C> c) {
        order.append(step);
        return c;
    }

    static <T> String narrow(Class<?> c, Class<T> to) {
        try {
            Class<? extends T> narrowed = traced("c", c).asSubclass(traced("to", to));
            return "as " + narrowed.getSimpleName();
        } catch (ClassCastException e) {
            return "cast: " + e.getMessage();
        } catch (NullPointerException e) {
            return "null";
        }
    }

    public static void main(String[] args) {
        Registry shapes = new Registry(Shape.class);
        Registry named = new Registry(Named.class);
        Class<?>[] tested = {Circle.class, Ring.class, Square.class, Suit.class, String.class, Shape.class};
        for (Class<?> c : tested) {
            System.out.println(shapes.accept(c) + ", " + named.accept(c));
        }
        System.out.println(shapes.base.isAssignableFrom(Suit.SPADES.getClass()) + " "
                + named.base.isAssignableFrom(Suit.SPADES.getClass()));

        Object[] values = {new Circle(), new Ring(), new Square("sq"), "text", 3, Suit.HEARTS, Suit.SPADES};
        System.out.println(ofType(values, Named.class) + " " + ofType(values, Circle.class) + " "
                + ofType(values, Number.class) + " " + ofType(values, Comparable.class) + " "
                + ofType(values, Object.class) + " " + ofType(values, Suit.class));

        StringBuilder table = new StringBuilder();
        for (int i = 0; i < 19; i++) {
            for (int j = 0; j < 19; j++) {
                table.append(pick(i).isAssignableFrom(pick(j)) ? '1' : '.');
            }
            table.append('\n');
        }
        System.out.print(table);

        System.out.println(Number.class.isAssignableFrom(Integer.class) + " " + Number.class.isAssignableFrom(String.class)
                + " " + Enum.class.isAssignableFrom(Suit.class) + " " + Suit.class.isAssignableFrom(Suit.SPADES.getClass()));
        Class<?> none = args.length > 5 ? String.class : null;
        System.out.println(narrow(Ring.class, Circle.class) + ", " + narrow(Suit.class, Named.class)
                + ", " + narrow(Square.class, Circle.class) + ", " + narrow(int.class, Object.class)
                + ", " + narrow(Integer.class, Comparable.class) + ", " + narrow(none, Shape.class)
                + ", " + narrow(Ring.class, none) + " " + order);
        try {
            System.out.println(Shape.class.isAssignableFrom(none));
        } catch (NullPointerException e) {
            System.out.println("null argument");
        }
        try {
            System.out.println(none.isAssignableFrom(String.class));
        } catch (NullPointerException e) {
            System.out.println("null class");
        }
    }
}
