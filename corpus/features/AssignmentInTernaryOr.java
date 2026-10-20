public class AssignmentInTernaryOr {
    static boolean sBoolean = true;
    static boolean[] aBoolean = new boolean[6];
    public static void main(String[] args) {
        boolean v4 = true;
        boolean b = aBoolean[0] ? ((sBoolean = v4) || true) : false;
        System.out.println(b + " " + sBoolean);
        aBoolean[1] = true;
        boolean c = aBoolean[1] ? ((sBoolean = false) || aBoolean[1]) : true;
        boolean d = aBoolean[1] ? ((sBoolean &= true) && v4) : false;
        System.out.println(c + " " + d + " " + sBoolean);
    }
}
