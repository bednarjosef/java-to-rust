public class FloatIncrementValue {
    static float sFloat = 1.5f;
    static float sBig = 16777216f;
    public static void main(String[] args) {
        System.out.println(++sFloat);
        System.out.println(sFloat-- + " " + sFloat);
        System.out.println(++sBig + " " + --sBig);
    }
}
