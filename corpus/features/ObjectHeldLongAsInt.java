// A cast of an `Object` holding a `Long` to `int` fails with a ClassCastException, which a catch
// clause sees with HotSpot's message; uncaught, it ends the program with exit code 1.
public class ObjectHeldLongAsInt {
    public static void main(String[] args) {
        Object o = 42L;
        try {
            int i = (int) o;
            System.out.println(i);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        long fine = (long) o;
        System.out.println(fine);
        int j = (int) o;
        System.out.println(j);
    }
}
