// A generic class that calls `Object`'s `equals`, which it inherits, on its objects and passes
// them nowhere else as `Object`s: the class is erased, as its objects become `Object`s.
public class GenericInheritedEquals {
    static class Holder<T> {
        T val;
        Holder(T val) {
            this.val = val;
        }
        T get() {
            return val;
        }
        void set(T v) {
            val = v;
        }
    }
    static class Point {
        int x;
        Point(int x) {
            this.x = x;
        }
    }
    static Object sObj = "s";

    public static void main(String[] args) {
        Holder<String> text = new Holder<>("Zed");
        text.set("x");
        Holder<Point> point = new Holder<>(new Point(3));
        System.out.println(text.get() + " " + text.equals("ab") + " " + point.get().x);
        System.out.println(point.equals(new Point(3)) + " " + text.equals(sObj));
        Holder<String> same = text;
        System.out.println(same == text);
    }
}
