public class NegatedUnsignedShiftCount {
    static long sLong = 5L;
    public static void main(String[] args) {
        System.out.println(1 >>> (-(0)));
        System.out.println((8L >> -(1)) + " " + (sLong << ~(2)) + " " + (-1 >>> -(31)));
    }
}
