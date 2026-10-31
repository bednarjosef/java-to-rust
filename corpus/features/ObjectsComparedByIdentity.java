// `==` on values held as `Object`s of many kinds: enum constants, `Boolean` and `Byte` (one object
// per value), the program's own objects and records, arrays and a string, each against every
// other; and a search by identity among them all.
public class ObjectsComparedByIdentity {
    enum Dir { NORTH, SOUTH }
    record Point(int x, int y) { }
    static final class Box {
        final int value;
        Box(int value) { this.value = value; }
    }

    static int indexOf(Object[] all, Object key) {
        for (int i = 0; i < all.length; i++) {
            if (all[i] == key) {
                return i;
            }
        }
        return -1;
    }

    public static void main(String[] args) {
        Box box = new Box(1);
        Point point = new Point(1, 2);
        int[] ints = { 1 };
        Object[] all = { Dir.NORTH, Dir.SOUTH, Boolean.TRUE, Boolean.FALSE, (byte) 3, (byte) 4, box,
            new Box(1), point, new Point(1, 2), ints, new int[] { 1 }, "text" };
        // The same values again, the boxes made anew: `valueOf` gives the same objects.
        Object[] again = { Dir.NORTH, Dir.SOUTH, Boolean.valueOf(args.length == 0), Boolean.valueOf(args.length > 0),
            Byte.valueOf((byte) (3 + args.length)), Byte.valueOf((byte) 4), box, new Box(1), point,
            new Point(1, 2), ints, new int[] { 1 }, null };
        for (int i = 0; i < all.length; i++) {
            StringBuilder line = new StringBuilder();
            for (int j = 0; j < again.length; j++) {
                // A string against itself is not compared: the translation does not keep a
                // string's identity.
                if (all[i] instanceof String && again[j] instanceof String) {
                    line.append('-');
                    continue;
                }
                line.append(all[i] == again[j] ? '1' : '0');
            }
            System.out.println(line);
        }
        System.out.println(indexOf(all, point) + " " + indexOf(all, box) + " " + indexOf(all, Byte.valueOf((byte) 4))
            + " " + indexOf(all, new Point(1, 2)) + " " + indexOf(all, Dir.SOUTH) + " " + indexOf(all, null));
        Object o = args.length > 3 ? null : Dir.NORTH;
        Object p = args.length > 3 ? Dir.NORTH : null;
        System.out.println((o == p) + " " + (o == Dir.NORTH) + " " + (p == null) + " " + (all[6] == again[6]));
    }
}
