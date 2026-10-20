public class CompoundEvaluationOrder {
    static int counter = 1;
    int field = 1;
    static int next() { counter += 10; return counter; }
    int bump() { field += 100; return 1; }
    static int twice(int v) { return v * 2; }
    public static void main(String[] args) {
        int y = 2;
        y += y++ + ++y;
        System.out.println(y);
        int z = 3;
        z *= (z = 5) + 1;
        System.out.println(z);
        counter += next();
        System.out.println(counter);
        CompoundEvaluationOrder o = new CompoundEvaluationOrder();
        o.field += o.bump();
        System.out.println(o.field);
        int[] arr = {1, 2, 3};
        int k = 0;
        arr[k] += (k = 2) + arr[k];
        System.out.println(arr[0] + " " + k);
        int w = 7;
        w -= w = 1;
        System.out.println(w);
        int q = 4;
        q >>= (q = 1);
        System.out.println(q);
        long lg = 2;
        lg += lg++ * 10;
        System.out.println(lg);
        double dv = 1.5;
        dv += dv++ + 1;
        System.out.println(dv);
        byte bt = 3;
        bt += (bt = 100) / 2;
        System.out.println(bt);
        char ch = 'a';
        ch += ch++ - 'a';
        System.out.println(ch);
    }
}
