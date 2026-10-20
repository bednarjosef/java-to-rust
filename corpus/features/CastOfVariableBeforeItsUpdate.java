public class CastOfVariableBeforeItsUpdate {
    public static void main(String[] args) {
        long count = 5L;
        int k = 3;
        System.out.println("count " + ((long) count) + " then " + (--count) + " and " + ((int) k) + " " + (k += 2));
        System.out.println(count + " " + k);
    }
}
