// Two subclasses of a class without toString each declare one, so their toString methods are
// two groups with numbered names; super.toString() still calls the Display one, to_string.
public class SeparateToStringGroups {
    static class Shape {
        int size = 2;
    }
    static class Square extends Shape {
        int side = 3;
        @Override
        public String toString() {
            return "Square(" + side + ", " + size + ")";
        }
    }
    static abstract class Round extends Shape {
        @Override
        public String toString() {
            return "Round(" + size + ")";
        }
    }
    static class Circle extends Round {
        @Override
        public String toString() {
            return "Circle/" + super.toString();
        }
    }
    static class Dot extends Circle {
        @Override
        public String toString() {
            String parent = super.toString();
            return "Dot/" + parent.length() + "/" + parent;
        }
    }

    public static void main(String[] args) {
        Shape s = new Square();
        Shape c = new Circle();
        Shape d = new Dot();
        System.out.println(s + " " + c + " " + d);
        s.size = 5;
        d.size = 7;
        System.out.println(s.toString() + " " + d.toString() + " " + c);
    }
}
