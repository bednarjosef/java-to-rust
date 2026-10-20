public class CompoundTargets {
    static int s = 4;
    int f = 4;
    static int bump() { s += 100; return 1; }
    int fbump() { f += 100; return 1; }
    public static void main(String[] args) {
        int[] a = {5, 6};
        a[0] += (a[0] = 50);
        System.out.println(a[0]);
        a[1] *= a[1]++ + 1;
        System.out.println(a[1]);
        a[0] += bump();
        System.out.println(a[0] + " " + s);
        s += (s = 9) * 2;
        System.out.println(s);
        s -= s++;
        System.out.println(s);
        CompoundTargets t = new CompoundTargets();
        t.f += (t.f = 1) + 1;
        System.out.println(t.f);
        t.f += t.fbump();
        System.out.println(t.f);
        short sh = 3;
        sh += (sh = 1000) * 40;
        System.out.println(sh);
        byte by = 1;
        by += (by = 127);
        System.out.println(by);
        float fl = 1f;
        fl += (fl = 2.5f) * 2;
        System.out.println(fl);
        long acc = 0;
        for (int i = 0; i < 5; i++) {
            acc += i * (i++ + 1);
        }
        System.out.println(acc);
    }
}
