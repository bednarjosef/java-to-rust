// What a class value tells without reflection's table: canonical and package names (null for
// local and anonymous classes), the constants of an enum, the interfaces a class names, and
// whether one class or interface is assignable from another.
public class ClassNamesAndMembers {
    interface Shape {
        double area();
    }

    interface Named {
        String name();
    }

    static class Square implements Shape, Named {
        public double area() {
            return 4;
        }

        public String name() {
            return "square";
        }
    }

    static class Tile extends Square {
    }

    enum Suit { CLUBS, DIAMONDS, HEARTS, SPADES }

    static void describe(Class<?> c) {
        System.out.println(c.getName() + " | " + c.getSimpleName() + " | " + c.getCanonicalName() + " | ["
                + c.getPackageName() + "] | " + c.isAnnotation());
    }

    public static void main(String[] args) {
        class Local {
        }
        Object anonymous = new Object() {
        };
        describe(Square.class);
        describe(Local.class);
        describe(anonymous.getClass());
        describe(Square[].class);
        describe(int[][].class);
        describe(String.class);
        describe(int.class);
        for (Suit s : Suit.class.getEnumConstants()) {
            System.out.print(s + " ");
        }
        System.out.println();
        Class<?> suit = Suit.HEARTS.getClass();
        System.out.println(suit.getEnumConstants().length);
        System.out.println(Square.class.getEnumConstants() == null);
        for (Class<?> i : Square.class.getInterfaces()) {
            System.out.println("interface " + i.getName());
        }
        Class<?>[] none = Tile.class.getInterfaces();
        System.out.println(none.length);
        System.out.println(Shape.class.isAssignableFrom(Tile.class) + " " + Named.class.isAssignableFrom(Square.class)
                + " " + Shape.class.isAssignableFrom(Suit.class) + " " + Square.class.isAssignableFrom(Tile.class)
                + " " + Tile.class.isAssignableFrom(Square.class));
    }
}
