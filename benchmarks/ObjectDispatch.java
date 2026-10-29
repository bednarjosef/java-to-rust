// Objects of many classes behind one type, as a large program holds them: the values of block
// properties (enums behind `Comparable` and `Object`) looked up by identity in a palette that also
// holds strings, cast back to their enum, and counted by ordinal through `Enum`. Each of these
// is a test of the object's class among every class of the type, which the translation makes
// once per value; with 48 enums and the strings that test is what the loop is about.
public class ObjectDispatch {
    interface Prop {
        int weight();
    }

    enum Kind00 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 0 + ordinal(); } }
    enum Kind01 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 1 + ordinal(); } }
    enum Kind02 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 2 + ordinal(); } }
    enum Kind03 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 3 + ordinal(); } }
    enum Kind04 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 4 + ordinal(); } }
    enum Kind05 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 5 + ordinal(); } }
    enum Kind06 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 6 + ordinal(); } }
    enum Kind07 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 7 + ordinal(); } }
    enum Kind08 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 8 + ordinal(); } }
    enum Kind09 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 9 + ordinal(); } }
    enum Kind10 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 10 + ordinal(); } }
    enum Kind11 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 11 + ordinal(); } }
    enum Kind12 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 12 + ordinal(); } }
    enum Kind13 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 13 + ordinal(); } }
    enum Kind14 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 14 + ordinal(); } }
    enum Kind15 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 15 + ordinal(); } }
    enum Kind16 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 16 + ordinal(); } }
    enum Kind17 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 17 + ordinal(); } }
    enum Kind18 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 18 + ordinal(); } }
    enum Kind19 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 19 + ordinal(); } }
    enum Kind20 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 20 + ordinal(); } }
    enum Kind21 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 21 + ordinal(); } }
    enum Kind22 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 22 + ordinal(); } }
    enum Kind23 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 23 + ordinal(); } }
    enum Kind24 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 24 + ordinal(); } }
    enum Kind25 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 25 + ordinal(); } }
    enum Kind26 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 26 + ordinal(); } }
    enum Kind27 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 27 + ordinal(); } }
    enum Kind28 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 28 + ordinal(); } }
    enum Kind29 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 29 + ordinal(); } }
    enum Kind30 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 30 + ordinal(); } }
    enum Kind31 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 31 + ordinal(); } }
    enum Kind32 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 32 + ordinal(); } }
    enum Kind33 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 33 + ordinal(); } }
    enum Kind34 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 34 + ordinal(); } }
    enum Kind35 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 35 + ordinal(); } }
    enum Kind36 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 36 + ordinal(); } }
    enum Kind37 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 37 + ordinal(); } }
    enum Kind38 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 38 + ordinal(); } }
    enum Kind39 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 39 + ordinal(); } }
    enum Kind40 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 40 + ordinal(); } }
    enum Kind41 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 41 + ordinal(); } }
    enum Kind42 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 42 + ordinal(); } }
    enum Kind43 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 43 + ordinal(); } }
    enum Kind44 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 44 + ordinal(); } }
    enum Kind45 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 45 + ordinal(); } }
    enum Kind46 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 46 + ordinal(); } }
    enum Kind47 implements Prop { NORTH, EAST, SOUTH, WEST; public int weight() { return 47 + ordinal(); } }

    static Object[] palette;

    // The index of `key` among the palette's objects, by identity (a palette of block states).
    static int idFor(Object key) {
        for (int i = 0; i < palette.length; i++) {
            if (palette[i] == key) {
                return i;
            }
        }
        return -1;
    }

    static int ordinalOf(Enum<?> e) {
        return e.ordinal();
    }

    static long run(Comparable<?>[] values, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < values.length; i++) {
                Comparable<?> value = values[i];
                sum += idFor(value);
                sum += ordinalOf((Enum<?>) value);
                if (i % 48 == 7) {
                    sum += ((Kind07) value).weight();
                } else if (i % 48 == 40) {
                    sum += ((Kind40) value).weight();
                } else {
                    sum += ((Prop) value).weight();
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        Object[][] all = { Kind00.values(), Kind01.values(), Kind02.values(), Kind03.values(), Kind04.values(), Kind05.values(), Kind06.values(), Kind07.values(), Kind08.values(), Kind09.values(), Kind10.values(), Kind11.values(), Kind12.values(), Kind13.values(), Kind14.values(), Kind15.values(), Kind16.values(), Kind17.values(), Kind18.values(), Kind19.values(), Kind20.values(), Kind21.values(), Kind22.values(), Kind23.values(), Kind24.values(), Kind25.values(), Kind26.values(), Kind27.values(), Kind28.values(), Kind29.values(), Kind30.values(), Kind31.values(), Kind32.values(), Kind33.values(), Kind34.values(), Kind35.values(), Kind36.values(), Kind37.values(), Kind38.values(), Kind39.values(), Kind40.values(), Kind41.values(), Kind42.values(), Kind43.values(), Kind44.values(), Kind45.values(), Kind46.values(), Kind47.values() };
        palette = new Object[2 + 4 * all.length];
        palette[0] = "air";
        palette[1] = "stone";
        int n = 2;
        for (Object[] kind : all) {
            for (Object constant : kind) {
                palette[n++] = constant;
            }
        }
        // One constant of each enum, in the order of the enums.
        Comparable<?>[] values = new Comparable<?>[48];
        for (int k = 0; k < 48; k++) {
            values[k] = (Comparable<?>) all[k][k % 4];
        }
        String probe = args.length > 0 ? args[0] : "dirt";
        long check = idFor(probe);
        for (int warm = 0; warm < 2; warm++) {
            check += run(values, 20000);
        }
        long start = System.nanoTime();
        long result = run(values, 200000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("object dispatch result " + result + " " + check);
        System.err.println("kernel-ms " + elapsed);
    }
}
