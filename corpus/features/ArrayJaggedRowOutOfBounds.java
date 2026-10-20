// A row shorter than the first one: bounding the column by the first row's length must not
// remove the check on the others.
public class ArrayJaggedRowOutOfBounds {
    public static void main(String[] args) {
        int[][] m = {{1, 2, 3}, {4, 5}};
        int sum = 0;
        for (int i = 0; i < m.length; i++) {
            for (int j = 0; j < m[0].length; j++) {
                sum += m[i][j];
                System.out.println("sum " + sum);
            }
        }
        System.out.println("not reached " + sum);
    }
}
