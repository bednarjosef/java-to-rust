public class Boxing {
    static Integer twice(Integer x) {
        return x * 2;
    }

    static int plusOne(int x) {
        return x + 1;
    }

    static Integer maybe(boolean present) {
        if (present) {
            return 7;
        }
        return null;
    }

    static Long big(long v) {
        Long boxed = v;
        return boxed;
    }

    static Double half(Double d) {
        return d / 2;
    }

    static Character next(Character c) {
        return (char) (c + 1);
    }

    public static void main(String[] args) {
        Integer a = 5;
        int b = a;
        Integer c = b + 10;
        System.out.println(a + " " + b + " " + c);
        System.out.println(twice(a) + " " + plusOne(a));
        System.out.println(a < c);
        System.out.println(c - a == b * 2);

        Integer counter = 0;
        for (int i = 0; i < 5; i++) {
            counter++;
            counter += i;
        }
        System.out.println("counter=" + counter);

        Integer none = maybe(false);
        System.out.println("none=" + none);
        System.out.println(none == null);
        Integer some = maybe(true);
        System.out.println("some=" + some + " " + (some != null));
        System.out.println(some.equals(7) + " " + some.equals(8));

        Long l = big(1L << 40);
        System.out.println(l + " " + l.intValue() + " " + l.hashCode());
        System.out.println(half(3.0) + " " + half(-1.5));
        Character ch = next('a');
        System.out.println(ch + " " + (int) ch);

        Boolean flag = a > 3;
        if (flag) {
            System.out.println("flag set " + flag.toString());
        }
        Short sh = 12;
        Byte by = (byte) -3;
        Float fl = 1.5f;
        System.out.println(sh + by + " " + fl * 2);

        int total = 0;
        Integer missing = null;
        total += missing;
        System.out.println("unreachable " + total);
    }
}
