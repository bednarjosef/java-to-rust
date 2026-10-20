import java.util.Arrays;

public class Prog {
    static boolean sBoolean = true;
    static float[] aFloat = new float[1];
    public static void main(String[] args) {
        aFloat[Math.floorMod(Math.abs((sBoolean ? 2 : -1)), aFloat.length)]++;
    }
}
