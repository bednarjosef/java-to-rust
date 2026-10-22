// A primitive boxed into an `Object` comes back out as the primitive it was, for each of the eight
// kinds: the cast from `Object` goes through the box, and the box is the one the `Object` holds.
public class ObjectHeldPrimitives {
    static Object pick(boolean first, Object a, Object b) {
        return first ? a : b;
    }

    public static void main(String[] args) {
        Object z = true;
        Object c = 'x';
        Object b = (byte) -7;
        Object s = (short) 300;
        Object i = 42;
        Object l = 9000000000L;
        Object f = 1.5f;
        Object d = 2.5;
        boolean zb = (boolean) z;
        char cc = (char) c;
        byte bb = (byte) b;
        short ss = (short) s;
        int ii = (int) i;
        long ll = (long) l;
        float ff = (float) f;
        double dd = (double) d;
        System.out.println(zb + " " + cc + " " + bb + " " + ss + " " + ii + " " + ll + " " + ff + " " + dd);
        System.out.println((char) c + 1);
        System.out.println((int) i + (long) l);
        System.out.println((char) pick(true, c, z) + "" + (boolean) pick(false, c, z));
        Object back = (char) c;
        System.out.println(back + " " + back.equals('x') + " " + ((char) back == 'x'));
    }
}
