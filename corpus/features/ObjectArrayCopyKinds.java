// `System.arraycopy` between arrays of many kinds held as `Object`s: arrays of arrays, of enums,
// of interfaces, of classes with subclasses, of strings and boxes. HotSpot checks each element as
// it is stored, stores the ones before the first that fails, and then throws, with a message
// that tells whether the destination's element type is below the source's.
public class ObjectArrayCopyKinds {
    enum Color { RED, GREEN, BLUE }

    enum Size { SMALL, LARGE }

    interface Shape {
        double area();
    }

    interface Tagged {}

    interface Named {
        String name();
    }

    record Circle(double r) implements Shape, Named {
        public double area() {
            return 3 * r * r;
        }

        public String name() {
            return "circle";
        }
    }

    record Square(double side) implements Shape, Tagged {
        public double area() {
            return side * side;
        }
    }

    static class Animal {
        final String sound;

        Animal(String sound) {
            this.sound = sound;
        }

        public String toString() {
            return getClass().getSimpleName() + "(" + sound + ")";
        }
    }

    static class Dog extends Animal {
        Dog() {
            super("woof");
        }
    }

    static class Cat extends Animal {
        Cat() {
            super("meow");
        }
    }

    // Each copy goes through these `Object`s, as the JDK's generic code does.
    static void copy(String what, Object src, int from, Object dest, int to, int n) {
        try {
            System.arraycopy(src, from, dest, to, n);
            System.out.println(what + ": " + show(dest));
        } catch (ArrayStoreException | IndexOutOfBoundsException e) {
            System.out.println(what + ": " + e.getClass().getSimpleName() + ": " + e.getMessage());
            System.out.println("  after: " + show(dest));
        }
    }

    // Any array as an array of its own class, each element shown as an `Object`.
    static String show(Object a) {
        StringBuilder sb = new StringBuilder("[");
        if (a instanceof Object[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof String[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Integer[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof CharSequence[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Number[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Comparable[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Color[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Size[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Enum[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Circle[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Shape[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Named[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Animal[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Dog[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Cat[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof Object[][] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof String[][] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof int[][] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof long[][] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof int[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else if (a instanceof long[] x) {
            for (int i = 0; i < x.length; i++) {
                sb.append(i == 0 ? "" : ", ").append(show(x[i]));
            }
        } else {
            return String.valueOf(a);
        }
        return sb.append("]").toString();
    }

    public static void main(String[] args) {
        // Strings and objects.
        copy("strings to objects", new String[] {"a", "b", null}, 0, new Object[4], 1, 3);
        copy("objects to strings", new Object[] {"x", null, "y"}, 0, new String[3], 0, 3);
        copy("objects to strings, failing", new Object[] {"x", "y", 7, "z"}, 0, new String[4], 0, 4);
        copy("integers to strings, nulls", new Integer[] {null, null}, 0, new String[2], 0, 2);
        copy("integers to strings", new Integer[] {null, 1, 2}, 0, new String[] {"p", "q", "r"}, 0, 3);
        copy("strings to char sequences", new String[] {"cs"}, 0, new CharSequence[2], 1, 1);
        copy("char sequences to strings", new CharSequence[] {"s", new StringBuilder("b")}, 0, new String[2], 0, 2);

        // Boxes.
        copy("integers to numbers", new Integer[] {1, 2}, 0, new Number[2], 0, 2);
        copy("numbers to integers", new Number[] {3, 4.5, 5}, 0, new Integer[3], 0, 3);
        copy("integers to comparables", new Integer[] {8}, 0, new Comparable[1], 0, 1);

        // Enums.
        copy("colors to objects", new Color[] {Color.RED, Color.BLUE}, 0, new Object[2], 0, 2);
        copy("objects to colors", new Object[] {Color.GREEN, null}, 0, new Color[2], 0, 2);
        copy("objects to colors, failing", new Object[] {Color.GREEN, "red"}, 0, new Color[2], 0, 2);
        copy("colors to sizes", new Color[] {null, Color.RED}, 0, new Size[2], 0, 2);
        copy("colors to enums", new Color[] {Color.BLUE}, 0, new Enum[1], 0, 1);
        copy("enums to colors", new Enum[] {Color.RED, Size.SMALL}, 0, new Color[2], 0, 2);

        // Interfaces and classes.
        copy("circles to shapes", new Circle[] {new Circle(1)}, 0, new Shape[1], 0, 1);
        copy("shapes to circles", new Shape[] {new Circle(2), new Square(3), new Circle(4)}, 0, new Circle[3], 0, 3);
        copy("shapes to named", new Shape[] {new Circle(5), new Square(6)}, 0, new Named[2], 0, 2);
        copy("named to shapes", new Named[] {new Circle(7)}, 0, new Shape[1], 0, 1);
        copy("animals to dogs", new Animal[] {new Dog(), new Dog(), new Cat()}, 0, new Dog[3], 0, 3);
        copy("dogs to animals", new Dog[] {new Dog()}, 0, new Animal[2], 1, 1);
        copy("dogs to cats", new Dog[] {null, new Dog()}, 0, new Cat[2], 0, 2);
        copy("objects to animals", new Object[] {new Cat(), new Circle(1)}, 0, new Animal[2], 0, 2);
        copy("animals to shapes", new Animal[] {new Cat()}, 0, new Shape[1], 0, 1);

        // Arrays of arrays.
        copy("string rows to objects", new String[][] {{"d"}}, 0, new Object[1], 0, 1);
        copy("int rows to objects", new int[][] {{1, 2}, {3}}, 0, new Object[2], 0, 2);
        copy("objects to int rows", new Object[] {new int[] {4}, null}, 0, new int[2][], 0, 2);
        copy("objects to int rows, failing", new Object[] {new int[] {5}, "six", new int[] {7}}, 0, new int[3][], 0, 3);
        copy("int rows to long rows", new int[][] {null, {8}}, 0, new long[2][], 0, 2);
        copy("string rows to comparables", new String[][] {null, {"g"}}, 0, new Comparable[2], 0, 2);

        // An `Object[]` that is really a `Tagged[]`, which carries its class.
        Object[] held = new Tagged[] {null, new Square(9)};
        copy("tagged held as objects to shapes", held, 0, new Shape[2], 0, 2);
        copy("tagged held as objects to circles", held, 0, new Circle[2], 0, 2);
        copy("objects to tagged held as objects", new Object[] {"o", new Square(1)}, 0, held, 0, 2);
        copy("integers to tagged held as objects", new Integer[] {null, 2}, 0, held, 0, 2);

        // Bounds come first, before any element is looked at.
        copy("source past its end", new String[] {"h"}, 0, new Integer[3], 0, 2);
        copy("destination past its end", new Object[] {"i", "j"}, 0, new String[1], 0, 2);
        copy("negative position", new Color[1], -1, new Object[1], 0, 1);
        copy("negative length", new Shape[1], 0, new Circle[1], 0, -1);
        copy("nothing copied", new Object[] {1}, 0, new String[1], 0, 0);

        // Arrays of primitives and objects that are not arrays.
        copy("ints to objects", new int[] {1}, 0, new Object[1], 0, 1);
        copy("objects to ints", new Object[] {1}, 0, new int[1], 0, 1);
        copy("ints to longs", new int[] {1}, 0, new long[1], 0, 1);
        copy("string rows to ints", new String[1][], 0, new int[1], 0, 1);
        copy("not an array", "text", 0, new Object[1], 0, 1);
        copy("into not an array", new Object[1], 0, Color.RED, 0, 1);

        // One array into itself, and the same kind of array.
        Object[] row = {"k", "l", "m", null};
        copy("within one array", row, 0, row, 1, 3);
        copy("same kind", new Shape[] {new Square(2)}, 0, new Shape[2], 1, 1);
    }
}
