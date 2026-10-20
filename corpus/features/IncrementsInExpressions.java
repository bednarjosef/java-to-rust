public class IncrementsInExpressions {
    static int counter = 0;
    static int next() { return ++counter; }
    public static void main(String[] args) {
        int i = 5;
        int a = i++ + ++i;
        System.out.println(a + " " + i);
        int b = i-- - --i;
        System.out.println(b + " " + i);
        int c = i++ * i++;
        System.out.println(c + " " + i);
        int big = Integer.MAX_VALUE;
        System.out.println(big++);
        System.out.println(big);
        System.out.println(++big);
        int min = Integer.MIN_VALUE;
        System.out.println(min--);
        System.out.println(min);
        long l = Long.MAX_VALUE;
        System.out.println(l++ + " " + l);
        byte by = 127;
        System.out.println(by++ + " " + by);
        short sh = -32768;
        System.out.println(--sh);
        char ch = 'y';
        System.out.println(ch++);
        System.out.println(ch);
        System.out.println(++ch);
        double d = 0.5;
        System.out.println(d++ + " " + ++d);
        float f = 1.5f;
        f--;
        System.out.println(f);
        int[] arr = new int[3];
        int k = 0;
        arr[k++] = k++;
        arr[k] = next() + next() * next();
        System.out.println(arr[0] + " " + arr[1] + " " + arr[2] + " " + k + " " + counter);
        int x = 3;
        x = x++ + x++;
        System.out.println(x);
        int y = 2;
        y += y++ + ++y;
        System.out.println(y);
        Integer boxed = 5;
        boxed++;
        System.out.println(boxed);
        System.out.println(i + i++ * -i--);
    }
}
