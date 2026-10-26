// A compound assignment to a byte, short, char or float whose value is a box: the box is unboxed,
// the arithmetic is done in the promoted type, and the result is narrowed back.
public class CompoundBoxedIntoSmall {
    public static void main(String[] args) {
        Short[] shorts = {(short) 30000, (short) 5000};
        short s = shorts[0];
        s += shorts[1];
        System.out.println(s);
        s -= Short.valueOf((short) 7);
        s *= Integer.valueOf(3);
        System.out.println(s);
        byte b = 100;
        b += Integer.valueOf(100);
        b ^= Byte.valueOf((byte) 15);
        System.out.println(b);
        char c = 'a';
        c += Character.valueOf((char) 2);
        c += Integer.valueOf(1);
        System.out.println(c);
        float f = 0.1f;
        f += Double.valueOf(0.2);
        System.out.println(f);
        f *= Float.valueOf(3.0f);
        System.out.println(f);
        short t = 7;
        t /= Integer.valueOf(2);
        t %= Short.valueOf((short) 2);
        System.out.println(t);
        Short missing = null;
        try {
            s += missing;
        } catch (NullPointerException e) {
            System.out.println("null box: " + e.getClass().getName());
        }
    }
}
