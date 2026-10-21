// A `toString` inherited from an abstract class whose subclasses are an enum, shown for a value of
// the abstract type and for a value of one subclass's own type.
public class InheritedText {
    abstract static class Shape {
        abstract String name();

        public String toString() {
            return "shape " + name();
        }
    }

    static class Circle extends Shape {
        String name() {
            return "circle";
        }
    }

    static class Square extends Shape {
        String name() {
            return "square";
        }
    }

    public static void main(String[] args) {
        Circle c = new Circle();
        Shape s = new Square();
        System.out.println("c is " + c);
        System.out.println("s is " + s);
    }
}
