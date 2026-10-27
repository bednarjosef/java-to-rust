// `(C[][][]) Array.newInstance(C.class, a, b, c)`, as Minecraft's `BlockPatternBuilder` makes its
// grid of tests: a new array of several dimensions of a class that a literal names, each level
// filled with arrays of the next, and the failure of a negative length.
import java.lang.reflect.Array;
import java.util.function.Predicate;

public class ArrayNewInstanceDimensions {
    @SuppressWarnings("unchecked")
    static Predicate<String>[][][] grid(int depth, int height, int width) {
        return (Predicate<String>[][][]) Array.newInstance(Predicate.class, depth, height, width);
    }

    public static void main(String[] args) {
        Predicate<String>[][][] tests = grid(2, 3, 4);
        System.out.println(tests.length + " " + tests[1].length + " " + tests[1][2].length + " " + tests[0][0][0]);
        for (int d = 0; d < tests.length; d++) {
            for (int h = 0; h < tests[d].length; h++) {
                for (int w = 0; w < tests[d][h].length; w++) {
                    int length = d + h + w;
                    tests[d][h][w] = s -> s.length() == length;
                }
            }
        }
        System.out.println(tests[1][2][3].test("abcdef") + " " + tests[1][2][3].test("abc"));
        System.out.println(tests[0][1] != tests[1][1]);

        short rows = 2;
        char columns = 3;
        int[][] numbers = (int[][]) Array.newInstance(int.class, rows, columns);
        numbers[1][2] = 7;
        System.out.println(numbers.length + " " + numbers[1].length + " " + numbers[1][2] + " " + numbers[0][0]);
        String[][] empty = (String[][]) Array.newInstance(String.class, 0, 5);
        System.out.println(empty.length);
        String[] flat = (String[]) Array.newInstance(String.class, 3);
        System.out.println(flat.length + " " + flat[2]);

        try {
            System.out.println(grid(2, -1, 4).length);
        } catch (NegativeArraySizeException e) {
            System.out.println("negative: " + e.getMessage());
        }
        try {
            System.out.println(((String[][]) Array.newInstance(String.class, 0, -3)).length);
        } catch (NegativeArraySizeException e) {
            System.out.println("negative under an empty one: " + e.getMessage());
        }
    }
}
