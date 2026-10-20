public class CompoundThroughField {
    static class Box { int v; }
    static class Holder { Box box; Holder(Box b) { box = b; } }
    static void bump(Holder h) { h.box.v += 10; }
    public static void main(String[] args) {
        Holder h = new Holder(new Box());
        bump(h);
        System.out.println(h.box.v);
    }
}
