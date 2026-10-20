public class OverloadedPrimitives {
    static String show(int x) { return "int " + x; }
    static String show(long x) { return "long " + x; }
    static String show(double x) { return "double " + x; }
    static String show(char x) { return "char " + x; }
    static String pick(float x) { return "float " + x; }
    static String pick(double x) { return "double " + x; }
    static String small(short s) { return "short " + s; }
    static String small(byte b) { return "byte " + b; }
    static long twice(long v) { return v * 2; }
    static double half(double v) { return v / 2; }
    static char next(char c) { return (char) (c + 1); }
    static byte neg(byte b) { return (byte) -b; }
    static int field = 3;
    char letter;
    short count;
    float ratio;
    double precise;
    long total;
    boolean flag;
    public static void main(String[] args) {
        System.out.println(show('a'));
        System.out.println(show((short) 5));
        System.out.println(show(5L));
        System.out.println(show(5f));
        System.out.println(show('a' + 1));
        System.out.println(pick(1));
        System.out.println(pick(1L));
        System.out.println(pick(2.5f));
        System.out.println(small((byte) 1));
        System.out.println(small((short) 1));
        System.out.println(twice(field));
        System.out.println(half(field));
        System.out.println(next('y'));
        System.out.println(neg((byte) -128));
        System.out.println(next('y') + 1);
        OverloadedPrimitives o = new OverloadedPrimitives();
        System.out.println("[" + o.letter + "]" + (int) o.letter);
        System.out.println(o.count + " " + o.ratio + " " + o.precise + " " + o.total + " " + o.flag);
        o.count += 40000;
        o.ratio += 1;
        o.precise -= 0.5;
        o.total = o.total - 1;
        System.out.println(o.count + " " + o.ratio + " " + o.precise + " " + o.total);
        double[] ds = new double[2];
        char[] cs = new char[2];
        System.out.println(ds[1] + " " + (int) cs[0] + " " + cs.length);
        System.out.println(ds[0] == 0.0);
    }
}
