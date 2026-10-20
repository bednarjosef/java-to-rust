public class CountingLoops {
    static int calls = 0;

    static int bound(int n) {
        calls++;
        return n;
    }

    public static void main(String[] args) {
        int n = 4;
        for (int i = 0; i < n; i++) {
            System.out.print(i + " ");
        }
        System.out.println();

        int m = 5;
        for (int i = 0; i < m; i++) {
            if (i == 1) {
                m = 3;
            }
            System.out.print(i + " ");
        }
        System.out.println();

        for (int i = 0; i < 10; i++) {
            if (i == 2) {
                i += 3;
            }
            System.out.print(i + " ");
        }
        System.out.println();

        for (int i = 0; i < bound(3); i++) {
            System.out.print("call " + i + " ");
        }
        System.out.println("calls " + calls);

        for (int i = Integer.MAX_VALUE - 3; i <= Integer.MAX_VALUE - 1; i++) {
            System.out.println("top " + i);
        }
        int count = 0;
        for (int i = Integer.MAX_VALUE - 2; i <= Integer.MAX_VALUE; i++) {
            count++;
            if (i == Integer.MAX_VALUE) {
                break;
            }
        }
        System.out.println("count " + count);

        int seen = 0;
        for (int i = Integer.MAX_VALUE - 5; i < Integer.MAX_VALUE; i += 2) {
            System.out.println("wrap " + i);
            if (++seen == 5) {
                break;
            }
        }

        for (int i = 0; i < 10; i += 3) {
            if (i == 3) {
                continue;
            }
            System.out.print(i + " ");
        }
        System.out.println();

        String s = "abcdefg";
        for (int i = 0; i < s.length(); i += 2) {
            System.out.print(s.charAt(i));
        }
        System.out.println();

        for (int i = -3; i < 2; i += 2) {
            System.out.print(i + " ");
        }
        System.out.println();

        for (int i = 5; i < 2; i++) {
            System.out.println("never " + i);
        }
        for (int i = 3; i <= 2; i++) {
            System.out.println("never " + i);
        }
        for (int i = 2; i < 2; i += 3) {
            System.out.println("never " + i);
        }

        for (int i = s.length() - 1; i >= 0; i--) {
            System.out.print(s.charAt(i));
        }
        System.out.println();

        int[] a = {1, 2, 3};
        for (int i = a.length - 1; i >= 0; i--) {
            System.out.print(a[i] + " ");
        }
        System.out.println();

        for (int i = n - 1; i >= 0; i--) {
            System.out.print(i + " ");
        }
        System.out.println();

        String empty = "";
        for (int i = empty.length() - 1; i >= 0; i--) {
            System.out.println("never " + i);
        }

        int last = 3;
        for (int i = last - 1; i >= 1; i--) {
            last = 0;
            System.out.print(i + " ");
        }
        System.out.println("last " + last);

        for (int i = 10; i >= 0; i -= 4) {
            System.out.print(i + " ");
        }
        System.out.println();
        for (int i = 10; i > 0; i -= 3) {
            System.out.print(i + " ");
        }
        System.out.println();

        outer:
        for (int i = 0; i < 4; i++) {
            for (int j = 0; j < 4; j++) {
                if (j > i) {
                    continue outer;
                }
                if (i == 3) {
                    break outer;
                }
                System.out.print(i * 10 + j + " ");
            }
        }
        System.out.println();

        for (char c = 'a'; c <= 'e'; c++) {
            System.out.print(c);
        }
        System.out.println();

        long total = 0;
        for (long i = 1; i <= 5; i++) {
            total += i;
        }
        System.out.println("total " + total);
    }
}
