// Static initializers that read other statics which main changes before reading the first ones:
// Java initialized them before main, from the values the statics had then.
public class StaticInitializerReadsChangedStatic {
    static int base = 1;
    static int derived = base + 1;
    static int[] row = {base, base * 2};
    static long big = 5L;
    static float[][] grid = {{base, big}, {derived}};
    static String name = "n";
    static String label = name + derived;
    static int untouched = 3;
    static int fromUntouched = untouched * untouched;

    static void change() {
        big++;
        name = "changed";
    }

    public static void main(String[] args) {
        base = 10;
        change();
        System.out.println(derived + " " + row[0] + " " + row[1]);
        System.out.println(grid[0][0] + " " + grid[0][1] + " " + grid[1][0]);
        System.out.println(label + " " + name + " " + big);
        System.out.println(fromUntouched);
    }
}
