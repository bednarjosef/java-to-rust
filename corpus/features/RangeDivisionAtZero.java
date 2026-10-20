public class RangeDivisionAtZero {
    public static void main(String[] args) {
        int total = 0;
        for (int i = 1; i <= 5; i++) {
            int d = 5 - i;
            total += 100 / d;
            System.out.println(total);
        }
    }
}
