public class NegatedLongLiteral {
    public static void main(String[] args) {
        System.out.println(-(4294967296L));
        System.out.println(~(9223372036854775807L));
        System.out.println((byte) (~(9223372036854775807L)));
        System.out.println(-(-9223372036854775807L - 1L));
    }
}
