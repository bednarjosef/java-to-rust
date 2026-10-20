public class BlockExpressionBeforeCast {
    static char sChar = 'A';
    static double m0() {
        return (sChar++);
    }
    public static void main(String[] args) {
        System.out.println(m0());
    }
}
