/// Java enum constants, strings and objects of a class held as `Object` and compared with `==`
/// and `equals`: a constant is the same as another value only when it is that constant. (Two
/// equal strings are not compared with `==`, which the translation does not support.)
public class EnumConstantsAsObjects {
    enum Color { RED, GREEN, BLUE }

    enum Size { SMALL, LARGE }

    static final class Box {
        final int n;

        Box(int n) {
            this.n = n;
        }
    }

    static int same(Object[] all, Object x) {
        int n = 0;
        for (Object y : all) {
            if (y == x) {
                n++;
            }
        }
        return n;
    }

    static int equal(Object[] all, Object x) {
        int n = 0;
        for (Object y : all) {
            if (y.equals(x)) {
                n++;
            }
        }
        return n;
    }

    public static void main(String[] args) {
        Box box = new Box(1);
        String s = "s";
        Object[] all = { Color.RED, Color.GREEN, s, box, Color.RED, Size.SMALL, new Box(1), Color.BLUE };
        System.out.println(same(all, Color.RED) + " " + same(all, Color.BLUE) + " " + same(all, Size.SMALL)
                + " " + same(all, Size.LARGE));
        System.out.println(same(all, box) + " " + same(all, new Box(1)));
        System.out.println(equal(all, Color.GREEN) + " " + equal(all, box) + " " + equal(all, "s"));
        Object first = all[0];
        Object last = all[4];
        System.out.println((first == last) + " " + (first == all[1]) + " " + (all[3] == all[6]));
    }
}
