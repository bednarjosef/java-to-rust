public class RangeDivisionByZero {
    static int half(int v) { return v / 2; }
    public static void main(String[] args) {
        int base = 10;
        for (int i = 0; i < 4; i++) {
            int divisor = i * 2 - 3;
            System.out.println(base / divisor);
        }
        System.out.println("unreachable");
    }
}
