// `instanceof` of an array against an array type, as the JDK's key manager tests a certificate
// chain (`certs instanceof X509Certificate[]` of a `Certificate[]`) and copies it into an array of
// the narrower class when it is not one.
public class ArrayInstanceOfArray {
    static abstract class Shape {
        abstract String name();
    }

    static class Circle extends Shape {
        String name() {
            return "circle";
        }
    }

    static Circle[] circles(Shape[] shapes) {
        if (!(shapes instanceof Circle[])) {
            Shape[] copy = new Circle[shapes.length];
            System.arraycopy(shapes, 0, copy, 0, shapes.length);
            shapes = copy;
        }
        return (Circle[]) shapes;
    }

    public static void main(String[] args) {
        Shape[] made = args.length > 0 ? new Shape[] {new Circle()} : new Circle[] {new Circle()};
        Shape[] plain = new Shape[] {new Circle(), new Circle()};
        System.out.println((made instanceof Circle[]) + " " + (plain instanceof Circle[]));
        System.out.println(circles(made).length + " " + circles(plain).length + " " + circles(plain)[1].name());
        Object[] words = args.length > 1 ? new Object[0] : new String[] {"a"};
        System.out.println((words instanceof String[]) + " " + (words instanceof Object[]));
        Shape[] none = null;
        System.out.println(none instanceof Circle[]);
    }
}
