public class ArrayCloneInts {
    public static void main(String[] args) {
        int[] a = {1, 2, 3};
        int[] b = a.clone();
        b[0] = 9;
        System.out.println(a[0] + " " + b[0]);
    }
}
