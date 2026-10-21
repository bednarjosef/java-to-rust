public class InterfaceDefaults {
    interface Part {
        int weight();
        default String show() { return kind() + "(" + weight() + ")"; }
        default String kind() { return "part"; }
        default void attach(Board b) { b.add(this); }
        private int doubled() { return 2 * weight(); }
        default int heavy() { return doubled(); }
    }

    static class Board {
        Part[] parts = new Part[8];
        int n;
        void add(Part p) { parts[n++] = p; }
        int total() {
            int t = 0;
            for (int i = 0; i < n; i++) t += parts[i].weight();
            return t;
        }
    }

    static abstract class Metal implements Part {
        int grams;
        Metal(int grams) { this.grams = grams; }
        public int weight() { return grams; }
        void polish() { grams -= 1; }
    }

    static class Bolt extends Metal {
        Bolt() { super(5); }
        public String kind() { return "bolt"; }
    }

    static class Plate extends Metal {
        Plate(int g) { super(g); }
    }

    static class Glue implements Part {
        public int weight() { return 1; }
    }

    public static void main(String[] args) {
        Board b = new Board();
        Metal m1 = new Bolt();
        Metal m2 = new Plate(40);
        Part g = new Glue();
        m1.attach(b);
        m2.attach(b);
        g.attach(b);
        new Plate(7).attach(b);
        m1.polish();
        m2.polish();
        System.out.println(b.total() + " " + b.n);
        for (int i = 0; i < b.n; i++) System.out.println(b.parts[i].show() + " " + b.parts[i].heavy());
        System.out.println(m1.show() + " " + m2.kind() + " " + (b.parts[0] == m1) + " " + (b.parts[1] == m1));
    }
}
