public class EqualsHashCode {
    interface Shape {
        double area();
    }

    static class Circle implements Shape {
        final int r;
        Circle(int r) { this.r = r; }
        public double area() { return 3 * r * r; }
        @Override public boolean equals(Object o) {
            return o instanceof Circle c && c.r == r;
        }
        @Override public int hashCode() { return r * 17; }
    }

    static class Square implements Shape {
        int side;
        Square(int side) { this.side = side; }
        public double area() { return side * side; }
    }

    static abstract class Animal {
        String name;
        Animal(String name) { this.name = name; }
        @Override public boolean equals(Object other) {
            if (other == null) return false;
            if (!(other instanceof Animal)) return false;
            return name.equals(((Animal) other).name);
        }
        @Override public int hashCode() { return name.hashCode(); }
    }

    static class Dog extends Animal {
        Dog(String name) { super(name); }
        @Override public String toString() { return "Dog " + name; }
    }

    static class Cat extends Animal {
        int lives = 9;
        Cat(String name) { super(name); }
    }

    static int indexOf(Object[] items, Object x) {
        for (int i = 0; i < items.length; i++) {
            if (items[i].equals(x)) return i;
        }
        return -1;
    }

    static boolean same(Shape a, Shape b) {
        return a.equals(b);
    }

    public static void main(String[] args) {
        Shape a = new Circle(2);
        Shape b = new Circle(2);
        Shape s = new Square(3);
        System.out.println(same(a, b) + " " + same(a, s) + " " + same(s, s) + " " + same(s, new Square(3)));
        System.out.println(a.hashCode() + " " + (s.hashCode() == s.hashCode()));
        Animal d = new Dog("rex");
        Animal c = new Cat("rex");
        System.out.println(d.equals(c) + " " + c.equals(d) + " " + d.equals(null) + " " + (d.hashCode() == c.hashCode()));
        System.out.println(d + " " + d.hashCode());
        String text = c.toString();
        System.out.println(text.startsWith("EqualsHashCode$Cat@") + " " + text.endsWith(Integer.toHexString(c.hashCode())));
        Object[] items = { "one", 2, new Circle(3), new Dog("fido"), 'c', true, 5L };
        System.out.println(indexOf(items, "one") + " " + indexOf(items, 2) + " " + indexOf(items, new Circle(3))
            + " " + indexOf(items, new Dog("fido")) + " " + indexOf(items, 'c') + " " + indexOf(items, true)
            + " " + indexOf(items, 5) + " " + indexOf(items, 5L));
        int total = 0;
        for (Object item : items) {
            total += item.hashCode() % 1000;
            if (item instanceof Animal an) System.out.println("animal " + an.name);
            else if (item instanceof Character ch) System.out.println("char " + (char) (ch + 1));
        }
        System.out.println(total);
        System.out.println("one".equals(items[0]) + " " + items[1].equals(2) + " " + items[0].toString().length());
    }
}
