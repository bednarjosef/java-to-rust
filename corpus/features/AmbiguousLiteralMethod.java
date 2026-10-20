public class AmbiguousLiteralMethod {
    static boolean sBoolean = true;
    static float[] aFloat = new float[1];
    static char sChar = 'c';
    static short sShort = 7;
    public static void main(String[] args) {
        aFloat[Math.floorMod(Math.abs((sBoolean ? 2 : -1)), aFloat.length)]++;
        System.out.println(aFloat[0]);
        System.out.println((~(0)) - sChar);
        System.out.println(Math.abs(-(0)));
        System.out.println((sBoolean ? 9223372036854775807L : 2L) + sShort);
    }
}
