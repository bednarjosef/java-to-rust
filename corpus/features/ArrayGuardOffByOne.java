// A loop that runs one past the end of an array made with the loop's own bound must still throw.
public class ArrayGuardOffByOne {
    public static void main(String[] args) {
        int n = 4;
        int[] squares = new int[n];
        for (int i = 0; i <= n; i++) {
            squares[i] = i * i;
            System.out.println("wrote " + i);
        }
        System.out.println("not reached " + squares[0]);
    }
}
