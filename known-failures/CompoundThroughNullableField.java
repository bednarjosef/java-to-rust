public class CompoundThroughNullableField {
    static class Box { int v; }
    static class Holder { Box box; Holder(Box b) { box = b; } }
    static void bump(Holder h) { h.box.v += 10; }
    public static void main(String[] args) {
        Holder h = new Holder(new Box());
        bump(h);
        h.box = null;
        System.out.println(h.box == null);
    }
}
