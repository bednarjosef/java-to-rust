public class Arrays {
    public static void main(String[] args) {
        int[] squares = new int[5];
        for (int i = 0; i < squares.length; i++) {
            squares[i] = i * i;
        }
        int sum = 0;
        for (int i = 0; i < squares.length; i++) {
            sum += squares[i];
        }
        System.out.println("sum = " + sum + ", length = " + squares.length);
        int[] empty = new int[0];
        System.out.println("empty length = " + empty.length);
        squares[2] = 7;
        System.out.println(squares[2] + squares[4]);
        int[] same = squares;
        same[0] = 9;
        System.out.println(squares[0]);
    }
}
