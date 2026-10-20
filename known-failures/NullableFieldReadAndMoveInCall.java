public class NullableFieldReadAndMoveInCall {
    static class Box {
        final int v;
        Box(int v) { this.v = v; }
    }

    static Box find(int v) {
        return v > 0 ? new Box(v) : null;
    }

    static String show(Box b) {
        return b == null ? "none" : "box " + b.v;
    }

    public static void main(String[] args) {
        Box b = find(3);
        System.out.println(b.v + " " + show(b));
    }
}
