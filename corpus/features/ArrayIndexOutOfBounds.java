public class ArrayIndexOutOfBounds {
    public static void main(String[] args) {
        int[] a = {5, 6, 7};
        int sum = 0;
        for (int i = 0; i <= a.length; i++) {
            System.out.println("reading " + i);
            sum += a[i];
        }
        System.out.println("not reached: " + sum);
    }
}
