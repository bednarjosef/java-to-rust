import java.util.List;
import shapes.Shape;
import shapes.Shapes;

// Compiles against classes that only a jar on the class path holds: an interface with a constant
// that javac inlines, a record, an enum and a generic method.
public class Drawing {
    public static void main(String[] args) {
        List<Shapes.Square> squares = List.of(new Shapes.Square(1), new Shapes.Square(3));
        for (Shape shape : Shapes.largerThan(squares, Shape.SIDES_OF_SQUARE)) {
            System.out.println(shape.describe() + " " + Shapes.Unit.CM);
        }
    }
}
