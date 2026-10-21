public class InitializerBlocksAfterSuper {
    static abstract class Base {
        int a;
        { System.out.println("base block"); a = 1; }
        Base(int x) { System.out.println("base ctor " + x); a += x; }
    }
    static class Leaf extends Base {
        int b = 5;
        { System.out.println("leaf block " + a); b += a; }
        Leaf() { super(10); System.out.println("leaf ctor " + b); }
        Leaf(int y) { this(); System.out.println("leaf ctor y " + y); }
    }
    public static void main(String[] args) {
        Leaf l = new Leaf(3);
        System.out.println(l.a + " " + l.b);
    }
}
