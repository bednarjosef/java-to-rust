import java.lang.reflect.Array;
import java.util.Arrays;
import java.util.function.Predicate;

// Arrays of three dimensions whose rows are also `Object[]`s (passed to `Arrays.deepToString`),
// made with `new` and with a cast `Array.newInstance` of a class literal, as Minecraft's
// `BlockPatternBuilder` makes its pattern: each level's rows are arrays of their own class, which
// a store or a copy of another array into them checks.
public class DeepCovariantArrays {
    @SuppressWarnings("unchecked")
    static Predicate<String>[][][] pattern(int aisles, int rows, int cols) {
        return (Predicate<String>[][][]) Array.newInstance(Predicate.class, aisles, rows, cols);
    }

    static String classes(Object[] array) {
        Object row = array[0];
        Object cell = ((Object[]) row)[0];
        return array.getClass().getSimpleName() + " " + row.getClass().getSimpleName() + " "
                + cell.getClass().getSimpleName() + " " + (row instanceof Object[][]) + " "
                + (cell instanceof Object[][]);
    }

    static void store(Object[] array, Object value) {
        try {
            array[0] = value;
            System.out.println("stored " + value.getClass().getSimpleName());
        } catch (ArrayStoreException e) {
            System.out.println("cannot store " + e.getMessage());
        }
    }

    static void copy(Object[] from, Object[] to) {
        try {
            System.arraycopy(from, 0, to, 0, from.length);
            System.out.println("copied " + from.length);
        } catch (ArrayStoreException e) {
            System.out.println("cannot copy " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        Predicate<String>[][][] p = pattern(2, 3, 4);
        p[1][2][3] = String::isEmpty;
        System.out.println(p.length + " " + p[0].length + " " + p[0][0].length);
        System.out.println(p[1][2][3].test("") + " " + (p[0][0][0] == null));
        Object[] rows = p[1];
        System.out.println(rows.length + " " + Arrays.deepToString(p[0]).length());
        System.out.println(classes(p) + " " + p[0].getClass().getName() + " " + (p[0][1] != p[1][1]));
        store(p[0], new Object[1]);
        store(p[0], new Predicate<?>[2]);
        store(p, new String[1][1]);
        copy(new Object[] {new Object[1]}, p[0]);
        copy(new Object[] {new Predicate<?>[1], null}, p[1]);

        String[][][] words = new String[2][2][3];
        words[1][1][2] = "x";
        Object[] aisle = words[1];
        System.out.println(Arrays.deepToString(words) + " " + aisle.length);
        System.out.println(classes(words) + " " + words[1].getClass().getName());
        store(aisle, new Integer[1]);
        store(aisle, new String[] {"y"});
        System.out.println(Arrays.deepToString(words));
        Object[] partial = new Long[1][2][];
        System.out.println(Arrays.deepToString(partial) + " " + partial[0].getClass().getSimpleName());

        // An `Integer[][][]` that stays one, whose rows are `Object[]`s.
        Integer[][][] grid = new Integer[2][2][2];
        grid[1][1][1] = 7;
        Object[] plane = grid[1];
        System.out.println(grid.getClass().getSimpleName() + " " + grid[1].getClass().getSimpleName() + " "
                + grid[1][1].getClass().getSimpleName() + " " + plane.length + " " + grid[1][1][1]);
        store(plane, new Long[1]);
        store(plane, new Integer[] {8});
        System.out.println(grid[1][0][0] + " " + grid[0][0].length);

        try {
            pattern(1, -1, 2);
        } catch (NegativeArraySizeException e) {
            System.out.println("negative " + e.getMessage());
        }
        try {
            Object[] none = new String[0][-2][1];
            System.out.println(none.length);
        } catch (NegativeArraySizeException e) {
            System.out.println("negative " + e.getMessage());
        }
    }
}
