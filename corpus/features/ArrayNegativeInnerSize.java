// Every size of a new array of arrays is checked before anything is made.
public class ArrayNegativeInnerSize {
    static int calls = 0;

    static int size(int n) {
        calls++;
        System.out.println("size " + n);
        return n;
    }

    public static void main(String[] args) {
        int[][] ok = new int[size(2)][size(3)];
        ok[1][2] = 5;
        System.out.println(ok[1][2] + " " + calls);
        int[][] bad = new int[size(-2)][size(-3)];
        System.out.println("not reached " + bad.length);
    }
}
