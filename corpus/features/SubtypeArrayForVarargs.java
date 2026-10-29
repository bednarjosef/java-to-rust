// An array of a class passed for the varargs of its interface (`Item[]` for `ItemLike...`):
// javac passes the array itself, not as one element of a new array, and the two array types
// meet only there.
public class SubtypeArrayForVarargs {
    interface Shape {
        int area();
    }

    static final class Square implements Shape {
        final int side;

        Square(int side) {
            this.side = side;
        }

        public int area() {
            return side * side;
        }
    }

    static final class Group {
        final Shape[] shapes;

        Group(Shape... shapes) {
            this.shapes = shapes;
        }
    }

    static final Square[] SQUARES = {new Square(1), new Square(2), new Square(3)};

    static int total(String label, Shape... shapes) {
        int sum = 0;
        for (Shape s : shapes) {
            sum += s.area();
        }
        System.out.println(label + ": " + shapes.length + " shapes, the same array: " + (shapes == (Object) SQUARES));
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(total("array", SQUARES));
        System.out.println(total("packed", new Square(4), new Square(5)));
        System.out.println(total("none"));
        Group group = new Group(SQUARES);
        System.out.println(group.shapes.length + " " + (group.shapes == (Object) SQUARES));
    }
}
