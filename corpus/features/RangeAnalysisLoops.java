public class RangeAnalysisLoops {
    static int limit = Integer.MAX_VALUE - 2;
    public static void main(String[] args) {
        int sum = 0;
        for (int i = 0; i < 10; i++) {
            sum += i * i * 1000000;
        }
        System.out.println(sum);
        int j = 0;
        while (j < 5) {
            j += 1000000000;
            System.out.println(j);
            j++;
        }
        int k = Integer.MAX_VALUE - 3;
        for (int n = 0; n < 6; n++) {
            k = k + 1;
            System.out.println(k);
        }
        for (int d = -3; d < 3; d++) {
            if (d != 0) System.out.println(12 / d);
        }
        int e = -2;
        for (int t = 0; t < 5; t++) {
            e++;
            if (e == 0) {
                System.out.println("zero hit");
                continue;
            }
            System.out.println(100 % e);
        }
        int m = 0;
        for (int w = 1; w < 40; w *= 2) {
            m = m * 2 + w;
        }
        System.out.println(m);
        int big = limit;
        big++;
        big++;
        System.out.println(big);
        big++;
        System.out.println(big);
        int c = 0;
        do {
            c += 50000;
        } while (c * c > 0 && c < 300000);
        System.out.println(c);
        int p = 46341;
        System.out.println(p * p);
        int q = 46340;
        System.out.println(q * q);
        byte bb = 0;
        for (int x = 0; x < 300; x++) {
            bb++;
        }
        System.out.println(bb);
        int acc = 1;
        for (int z = 1; z <= 20; z++) {
            acc *= z;
        }
        System.out.println(acc);
        long lacc = 1;
        for (int z = 1; z <= 25; z++) {
            lacc *= z;
        }
        System.out.println(lacc);
        int[] arr = new int[4];
        for (int i = 0; i < arr.length; i++) {
            arr[i] = i * 1000000000;
        }
        System.out.println(arr[3]);
        int idx = 0;
        for (int i = 0; i < 4; i++) {
            idx += 1 << (i * 8);
        }
        System.out.println(idx);
        int neg = -1;
        for (int i = 0; i < 3; i++) {
            neg = neg >>> 1;
            neg = neg << 1 | 1;
        }
        System.out.println(neg);
    }
}
