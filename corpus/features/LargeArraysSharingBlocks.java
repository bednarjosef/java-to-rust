// Large arrays whose elements fill whole 64 KB blocks, so that each one's header ends in a block
// that the next large array or small objects share: arrays of numbers and of objects, some kept
// and some dropped, small objects made between them, and the kept ones read back after many
// collections. A large array whose last block is shared must keep its own last elements and the
// objects they hold, and free blocks it shares only once both sides are dead.
public class LargeArraysSharingBlocks {
    static final class Box {
        final long value;
        final String name;

        Box(long value, String name) {
            this.value = value;
            this.name = name;
        }
    }

    public static void main(String[] args) {
        Object[][] keptObjects = new Object[40][];
        long[][] keptLongs = new long[40][];
        int[][] keptInts = new int[40][];
        Box[] smallKept = new Box[4000];
        long checksum = 0;
        for (int round = 0; round < 200; round++) {
            // 8192 references: 64 KB of elements and the header.
            Object[] objects = new Object[8192];
            for (int i = 0; i < objects.length; i += 1) {
                objects[i] = new Box(round * 100_000L + i, "b" + (i % 97));
            }
            long[] longs = new long[16384];
            int[] ints = new int[16384];
            for (int i = 0; i < 16384; i++) {
                longs[i] = (long) round * i;
                ints[i] = round ^ i;
            }
            // The last elements, which are in the shared block.
            objects[8191] = new Box(-round, "last" + round);
            longs[16383] = round * 7L;
            ints[16383] = round * 11;
            // Small objects made between the large ones take the rooms their ends leave.
            for (int i = 0; i < 300; i++) {
                Box b = new Box(round + i, "s" + i);
                if (i % 15 == 0) {
                    smallKept[(round * 20 + i / 15) % smallKept.length] = b;
                }
            }
            if (round % 5 == 0) {
                keptObjects[round / 5] = objects;
                keptLongs[round / 5] = longs;
                keptInts[round / 5] = ints;
            }
            checksum += ((Box) objects[8191]).value + longs[16383] + ints[16383];
            // Garbage between rounds, so that collections run.
            for (int i = 0; i < 2000; i++) {
                checksum += new long[64 + i % 300].length;
            }
        }
        for (int k = 0; k < 40; k++) {
            Box last = (Box) keptObjects[k][8191];
            Box first = (Box) keptObjects[k][0];
            long sum = 0;
            for (Object o : keptObjects[k]) {
                sum += ((Box) o).value + ((Box) o).name.length();
            }
            checksum += sum + last.value + last.name.hashCode() + first.value;
            checksum += keptLongs[k][16383] + keptLongs[k][100] + keptInts[k][16383] + keptInts[k][7];
        }
        long small = 0;
        for (Box b : smallKept) {
            if (b != null) {
                small += b.value + b.name.length();
            }
        }
        System.out.println(checksum);
        System.out.println(small);
        System.out.println(((Box) keptObjects[39][8191]).name);
    }
}
