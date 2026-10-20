// Primitive static fields, which translate to `Cell`s: each read and write copies the value, and
// the update forms (`+=`, `++`, `--`) must behave as Java does, including when the right operand
// changes the same field and when the field is read during its class's initialization.
public class PrimitiveStatics {
    static int count;
    static long total = 7L;
    static char letter = 'a';
    static byte small = 120;
    static short medium = -300;
    static float ratio = 0.5f;
    static double scale = 2.25;
    static boolean flag;
    static int fromCall = compute(3);
    static int seen = 0;

    static int compute(int n) {
        seen++;
        return n * n + count;
    }

    static int bump() {
        count += 100;
        return 1;
    }

    static int next() {
        return count++;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 5; i++) {
            count += i;
            total *= 3;
            total -= i;
        }
        System.out.println(count + " " + total);

        int a = count++ + ++count;
        int b = count-- - --count;
        System.out.println(a + " " + b + " " + count);

        count += bump();
        System.out.println(count);

        count = count + next() * 2;
        System.out.println(count);

        letter++;
        letter += 2;
        System.out.println(letter + " " + (int) letter);

        small += 10;
        small++;
        medium *= 2;
        medium--;
        System.out.println(small + " " + medium);

        ratio *= 3;
        ratio -= 0.25f;
        scale /= 4;
        scale++;
        System.out.println(ratio + " " + scale);

        flag = !flag;
        flag = flag && count > 0;
        System.out.println(flag);

        System.out.println(fromCall + " " + seen);
        System.out.println(total % 1000);
        int zero = 0;
        System.out.println(count / zero);
    }
}
