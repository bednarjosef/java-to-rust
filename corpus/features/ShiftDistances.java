public class ShiftDistances {
    public static void main(String[] args) {
        int one = 1;
        System.out.println(one << 31);
        System.out.println(one << 32);
        System.out.println(one << 33);
        System.out.println(one << -1);
        System.out.println(one << -33);
        System.out.println(-16 >> 2);
        System.out.println(-16 >> 33);
        System.out.println(-16 >>> 28);
        System.out.println(-16 >>> 32);
        System.out.println(-16 >>> -1);
        long lo = 1L;
        System.out.println(lo << 63);
        System.out.println(lo << 64);
        System.out.println(lo << 65);
        System.out.println(lo << -1);
        System.out.println(-1L >>> 60);
        System.out.println(-1L >>> 64);
        System.out.println(-1L >> 100);
        int sh = 70;
        System.out.println(1L << sh);
        System.out.println(1 << sh);
        byte nb = -1;
        System.out.println(nb >>> 4);
        System.out.println(nb >> 4);
        short ns = -2;
        System.out.println(ns >>> 1);
        char c = '￿';
        System.out.println(c >>> 3);
        System.out.println(c >> 3);
        System.out.println(c << 20);
        byte shifted = (byte) (nb << 3);
        System.out.println(shifted);
        System.out.println((byte) -128 >>> 1);
        long mixed = 3;
        System.out.println(one << mixed);
        System.out.println(mixed << one);
        System.out.println(-1 >>> 0 + 1);
    }
}
