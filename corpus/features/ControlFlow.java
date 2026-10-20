public class ControlFlow {
    public static void main(String[] args) {
        for (int i = 1; i <= 15; i++) {
            if (i % 15 == 0) {
                System.out.println("FizzBuzz");
            } else if (i % 3 == 0) {
                System.out.println("Fizz");
            } else if (i % 5 == 0) {
                System.out.println("Buzz");
            } else {
                System.out.println(i);
            }
        }
        int n = 27, steps = 0;
        while (n != 1) {
            n = n % 2 == 0 ? n / 2 : 3 * n + 1;
            steps++;
        }
        System.out.println("collatz steps: " + steps);
        int k = 0;
        do {
            k += 3;
        } while (k < 10);
        System.out.println("k = " + k);
        int odd = 0;
        for (int i = 0; i < 20; i++) {
            if (i % 2 == 0) continue;
            if (i > 15) break;
            odd += i;
        }
        System.out.println("odd sum: " + odd);
        int j = 0;
        int evens = 0;
        do {
            j++;
            if (j % 2 == 1) continue;
            evens++;
        } while (j < 10);
        System.out.println("evens: " + evens);
        int count = 0;
        for (int a = 0, b = 10; a < b; a++, b--) {
            if (a == 2) continue;
            count += b - a;
        }
        System.out.println("count: " + count);
        int found = -1;

        for (int i = 2; i < 100; i++) {
            if (i * i > 50) {
                found = i;
                break;
            }
        }
        System.out.println("first square over 50: " + found);
        int t = 0;
        while (true) {
            t++;
            if (t >= 5) break;
        }
        for (;;) {
            t--;
            if (t == 0) break;
        }
        System.out.println("t = " + t);
        int limit = 5;
        int total = 0;
        for (int i = 0; i < limit; i++) {
            limit = limit > 3 ? limit - 1 : limit;
            total += i;
        }
        System.out.println("total = " + total + ", limit = " + limit);
        boolean flag = true;
        if (!flag) System.out.println("unreachable");
        else System.out.println("flag is " + flag);
        System.out.println(flag && 1 > 2 || !flag == false);
        System.out.println(true ^ flag);
        int y = 5;
        int z = y++ + ++y;
        System.out.println(y + " " + z + " " + y-- + " " + --y);
        int w;
        int v = w = 7;
        System.out.println(v + w);
        while ((w = w - 2) > 0) {
            System.out.print(w + " ");
        }
        System.out.println();
    }
}
