public class StaticUpdates {
    static int total;
    static double scale = 1.0;
    static void add(int v) { total += v; scale *= v; }
    public static void main(String[] args) {
        add(3); add(4);
        int v = 5;
        total -= v;
        System.out.println(total + " " + scale);
    }
}
