public class ConditionalPromotion {
    public static void main(String[] args) {
        boolean t = args.length == 0;
        boolean f = !t;
        char c = 'A';
        int i = 66;
        long l = 67L;
        double d = 2.5;
        float fl = 1.25f;
        short s = 7;
        byte b = -8;
        System.out.println(t ? c : i);
        System.out.println(f ? c : i);
        System.out.println(t ? c : 0);
        System.out.println(t ? c : s);
        System.out.println(t ? b : s);
        System.out.println(t ? i : l);
        System.out.println(t ? i : d);
        System.out.println(t ? fl : d);
        System.out.println(t ? l : fl);
        System.out.println(f ? 'x' : 120);
        System.out.println(t ? 'x' : (char) 121);
        System.out.println(t ? null : "text");
        System.out.println(t ? 1 : 'c');
        System.out.println(f ? 1.0f : 2);
        final int K = 100;
        byte kb = t ? (byte) K : 0;
        System.out.println(kb);
        char cc = t ? 'a' : 0;
        System.out.println(cc);
        System.out.println(t ? 'a' : 1000000);
        long m = (t ? 3 : 4) + (f ? 1L : 2);
        System.out.println(m);
        System.out.println(t ? 1 : 2L + i);
    }
}
