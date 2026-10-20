public class ArrayExceptions {
    public static void main(String[] args) {
        int[] a = new int[3];
        int i = 0;
        a[i++] = i;
        a[i++] += i * 10;
        System.out.println(a[0] + " " + a[1] + " " + i);
        int[] sized = new int[-a[0]];
        System.out.println("not reached: " + sized.length);
    }
}
