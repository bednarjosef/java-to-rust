// `Class.isInstance` of a field that may hold null or an object of several classes, as javac's
// `AnnotationProxyMaker` tests its value against the return type of the element's method.
public class InstanceOfClassValues {
    interface Shape {}

    record Circle(int radius) implements Shape {}

    record Square(int side) implements Shape {}

    static final class Holder {
        Object value;
        Class<?> expected;

        Holder(Class<?> expected) {
            this.expected = expected;
        }

        boolean fits() {
            return expected.isInstance(value);
        }
    }

    public static void main(String[] args) {
        Holder shapes = new Holder(Shape.class);
        Holder circles = new Holder(Circle.class);
        Holder texts = new Holder(String.class);
        Object[] values = {null, new Circle(1), new Square(2), "text", 3};
        for (Object v : values) {
            shapes.value = v;
            circles.value = v;
            texts.value = v;
            System.out.println(v + ": " + shapes.fits() + " " + circles.fits() + " " + texts.fits());
        }
    }
}
