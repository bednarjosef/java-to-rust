// An index out of bounds on the rows of a plain array of arrays, after some output.
public class ArrayRowOutOfBounds {
    static int[][] make(int n) {
        return new int[n][2];
    }

    public static void main(String[] args) {
        int[][] m = make(3);
        for (int i = 0; i < 5; i++) {
            m[i][1] = i;
            System.out.println("row " + i + " " + m[i][1]);
        }
    }
}
