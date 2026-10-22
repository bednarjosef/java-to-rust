// Calls of an abstract `toString` whose implementations may return null, and may throw.
public class ThrowingToStringCalls {
    static abstract class Shape {
        public abstract String toString();
    }

    static class Circle extends Shape {
        public String toString() {
            return "circle";
        }
    }

    static class Blank extends Shape {
        public String toString() {
            return null;
        }
    }

    static class Broken extends Shape {
        int[] sides = new int[0];

        public String toString() {
            return "broken " + sides[1];
        }
    }

    static abstract class Label {
        public abstract String toString();
    }

    static class Named extends Label {
        public String toString() {
            return "named";
        }
    }

    static class Unnamed extends Label {
        public String toString() {
            return null;
        }
    }

    public static void main(String[] args) {
        Label[] labels = {new Named(), new Unnamed()};
        for (Label l : labels) {
            String text = l.toString();
            System.out.println(text == null ? "no label" : text.length() + " " + text);
            System.out.println(l);
        }
        Shape[] shapes = {new Circle(), new Blank(), new Broken()};
        for (Shape s : shapes) {
            try {
                String text = s.toString();
                System.out.println(text == null ? "no text" : text.length() + " " + text);
            } catch (ArrayIndexOutOfBoundsException e) {
                System.out.println("caught " + e.getMessage());
            }
        }
    }
}
