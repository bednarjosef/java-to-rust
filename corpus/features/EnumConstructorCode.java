import java.util.Arrays;

// Enum constructors that run code: a loop, a call of a static helper, an argument that is a
// constant of another enum, an initializer that is not a constant, and a field that changes later.
public class EnumConstructorCode {
    enum Kind { SMALL, LARGE }

    enum Shape {
        TRIANGLE(3, Kind.SMALL),
        SQUARE(4, Kind.SMALL),
        HEXAGON(6, Kind.LARGE);

        private final int sides;
        private final int angleSum;
        private final String outline;
        private final Kind kind;
        private final int[] corners;
        private final String name = name().toLowerCase();
        private int uses;

        Shape(int sides, Kind kind) {
            this.sides = sides;
            this.kind = kind;
            int sum = 0;
            for (int i = 2; i < sides; i++) {
                sum += 180;
            }
            this.angleSum = sum;
            this.outline = repeat("*", sides);
            corners = new int[sides];
            for (int i = 0; i < sides; i++) {
                corners[i] = i * 360 / sides;
            }
        }

        private static String repeat(String s, int times) {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < times; i++) {
                sb.append(s);
            }
            return sb.toString();
        }

        int use() {
            uses++;
            return uses;
        }

        boolean isLarge() {
            return kind == Kind.LARGE;
        }
    }

    enum Level {
        LOW(1), MEDIUM(5), HIGH(10);

        private final int weight;
        private final int rank;

        Level(int weight) {
            this.weight = weight * scale();
            this.rank = ordinal() + 1;
        }

        private static int scale() {
            return 3;
        }
    }

    public static void main(String[] args) {
        // An instance method first, before anything else uses the enum.
        System.out.println(Shape.HEXAGON.isLarge());
        for (Shape s : Shape.values()) {
            System.out.println(s + " " + s.sides + " " + s.angleSum + " " + s.outline + " " + s.kind + " "
                    + Arrays.toString(s.corners) + " " + s.isLarge() + " " + s.name);
        }
        Shape.SQUARE.use();
        Shape.SQUARE.use();
        System.out.println(Shape.SQUARE.use() + " " + Shape.TRIANGLE.use() + " " + Shape.SQUARE.uses);
        Shape.HEXAGON.uses = 40;
        System.out.println(Shape.HEXAGON.use());
        int total = 0;
        for (Level l : Level.values()) {
            total += l.weight;
        }
        System.out.println(total + " " + Level.HIGH.weight + " " + Level.MEDIUM.rank);
    }
}
