import java.util.Arrays;

public class Prog {
    static boolean sBoolean = true;
    static boolean[] aBoolean = new boolean[6];
    public static void main(String[] args) {
        boolean v4 = true;
        boolean b = aBoolean[0] ? ((sBoolean = v4) || true) : false;
        System.out.println(b + " " + sBoolean);
    }
}
