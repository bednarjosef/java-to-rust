// `Object`'s `toString` shows the object's `hashCode`, here one that may throw inside a `try`
// that catches it: in a concatenation, a print, `String.valueOf`, an explicit `toString` and a
// builder's append; of a class alone and of classes under a common interface, null or not. As
// javac's keys of pairs, whose `hashCode`s the text of an `Object` may run.
public class CaughtHashCodeInText {
    interface Shape {
        int sides();
    }

    static class Key implements Shape {
        final int[] parts;

        Key(int[] parts) {
            this.parts = parts;
        }

        public int sides() {
            return parts.length;
        }

        @Override
        public int hashCode() {
            if (parts.length == 0) {
                throw new IllegalStateException("no parts");
            }
            return parts[0] * 31 + parts.length;
        }
    }

    static class Square implements Shape {
        public int sides() {
            return 4;
        }

        @Override
        public String toString() {
            return "square";
        }
    }

    static String direct(Key key) {
        try {
            return "key " + key;
        } catch (IllegalStateException e) {
            return "direct caught " + e.getMessage();
        }
    }

    static String shown(Shape shape) {
        try {
            return "shape " + shape + (shape == null ? "" : " of " + shape.sides());
        } catch (IllegalStateException e) {
            return "shown caught " + e.getMessage();
        }
    }

    static String explicit(Shape shape) {
        try {
            return shape.toString().replace('@', '#');
        } catch (IllegalStateException e) {
            return "explicit caught " + e.getMessage();
        }
    }

    static String valueOf(Object o) {
        try {
            return String.valueOf(o);
        } catch (IllegalStateException e) {
            return "valueOf caught " + e.getMessage();
        }
    }

    static String appended(Object o) {
        StringBuilder sb = new StringBuilder("[");
        try {
            sb.append(o);
        } catch (IllegalStateException e) {
            sb.append("appended caught ").append(e.getMessage());
        }
        return sb.append(']').toString();
    }

    static void printed(Shape shape) {
        try {
            System.out.println(shape);
        } catch (IllegalStateException e) {
            System.out.println("printed caught " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        Key empty = new Key(new int[0]);
        Key full = new Key(new int[] {3, 4});
        System.out.println(direct(empty));
        System.out.println(direct(full));
        Shape[] shapes = {empty, full, new Square(), args.length > 5 ? new Square() : null};
        for (Shape s : shapes) {
            System.out.println(shown(s));
            if (s != null) {
                System.out.println(explicit(s));
                printed(s);
            }
            System.out.println(valueOf(s));
            System.out.println(appended(s));
        }
    }
}
