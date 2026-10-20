public class CompoundIndexOrder {
    static int calls = 0;
    static int next() { return ++calls; }
    public static void main(String[] args) {
        int[] arr = {10, 20, 30, 40};
        int i = 0;
        arr[i++] += i;
        System.out.println(arr[0] + " " + i);
        arr[i++] += i * 10;
        System.out.println(arr[1] + " " + i);
        arr[i] += (i = 0) + 5;
        System.out.println(arr[2] + " " + i);
        int j = 1;
        arr[j] = j++ + arr[j];
        System.out.println(arr[1] + " " + j);
        arr[next()] += next();
        System.out.println(arr[1] + " " + arr[2] + " " + calls);
    }
}
