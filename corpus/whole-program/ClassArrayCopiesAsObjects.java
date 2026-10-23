// `System.arraycopy` between arrays of classes held as `Object`s, as the JDK's code that is
// generic over the kind of array writes it (`SpinedBuffer`'s `T_ARR`): each element is checked
// against the destination's class as it is stored, with HotSpot's messages. No array here is
// taken as an `Object[]` of another type, so each keeps its own type.
public class ClassArrayCopiesAsObjects {
    interface Shape {}

    static class Circle implements Shape {
        public String toString() {
            return "circle";
        }
    }

    static class Square implements Shape {
        public String toString() {
            return "square";
        }
    }

    /** The copy: nothing, or the message of its `ArrayStoreException`. */
    static String copy(Object src, Object dest, int n) {
        try {
            System.arraycopy(src, 0, dest, 0, n);
            return "";
        } catch (ArrayStoreException e) {
            return " " + e.getMessage();
        }
    }

    static String show(Shape[] values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(values[i]);
        }
        return sb.append("]").toString();
    }

    static String show(Circle[] values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(values[i]);
        }
        return sb.append("]").toString();
    }

    static String show(Square[] values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(values[i]);
        }
        return sb.append("]").toString();
    }

    static String show(String[] values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(values[i]);
        }
        return sb.append("]").toString();
    }

    static String show(Object[] values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            sb.append(i == 0 ? "" : ", ").append(values[i]);
        }
        return sb.append("]").toString();
    }

    public static void main(String[] args) {
        Circle c = new Circle();
        Square s = new Square();
        Object any = s;
        System.out.println("an object " + any + " " + (Object) c);
        Shape[] shapes = new Shape[3];
        String m = copy(new Circle[] {c, null}, shapes, 2);
        System.out.println("circles into shapes: " + show(shapes) + m);
        Circle[] circles = new Circle[3];
        m = copy(new Shape[] {c, s, c}, circles, 3);
        System.out.println("shapes into circles: " + show(circles) + m);
        Shape[] more = new Shape[3];
        m = copy(new Object[] {s, "text", c}, more, 3);
        System.out.println("objects into shapes: " + show(more) + m);
        Square[] squares = new Square[2];
        m = copy(new Circle[] {null, null}, squares, 2);
        System.out.println("nulls into squares: " + show(squares) + m);
        Square[] one = new Square[1];
        m = copy(new Circle[] {c}, one, 1);
        System.out.println("circles into squares: " + show(one) + m);
        Object[] objects = new Object[2];
        m = copy(new Shape[] {s, c}, objects, 2);
        System.out.println("shapes into objects: " + show(objects) + m);
        Square[] same = new Square[2];
        m = copy(new Square[] {s, s}, same, 2);
        System.out.println("squares into squares: " + show(same) + m);
        String[] strings = new String[2];
        m = copy(new Object[] {"a", c}, strings, 2);
        System.out.println("objects into strings: " + show(strings) + m);
        String[] named = new String[1];
        m = copy(new Circle[] {c}, named, 1);
        System.out.println("circles into strings: " + show(named) + m);
        Circle[] fromText = new Circle[1];
        m = copy(new String[] {"a"}, fromText, 1);
        System.out.println("strings into circles: " + show(fromText) + m);
    }
}
