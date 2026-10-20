class Grid {
    int w = 4;
    int h = w * 2;
    int[] cells = new int[w * h];
    int filled;
    Grid() { filled = 1; }
    Grid(int extra) { this(); filled += extra; }
    int area() { return cells.length + filled; }
}
public class FieldInitializers {
    public static void main(String[] args) {
        System.out.println(new Grid().area() + " " + new Grid(5).area());
    }
}
