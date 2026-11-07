// The Minecraft server's random block ticks pick a position in a section, read its x, y and z,
// and pass the position on only for the few blocks that tick. The position is an object of a
// class with a subclass (`BlockPos`, `MutableBlockPos`), so its handle carries its class in its
// top bits; it is made on the stack, and moved to the heap only on the path where it is kept.
// It runs on a worker thread, as the server's ticks do, so the objects' fields are ones that
// threads may share.
public class RandomTickPositions {
    static class Pos {
        private final int x;
        private final int y;
        private final int z;

        Pos(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        int getX() {
            return x;
        }

        int getY() {
            return y;
        }

        int getZ() {
            return z;
        }
    }

    static final class MutablePos extends Pos {
        MutablePos(int x, int y, int z) {
            super(x, y, z);
        }
    }

    static final class Xorshift {
        int seed = 12345;

        int next() {
            seed ^= seed << 13;
            seed ^= seed >>> 17;
            seed ^= seed << 5;
            return seed;
        }
    }

    static final Pos[] kept = new Pos[64];
    static int keptCount;

    static Pos randomPos(Xorshift random, int minX, int minY, int minZ) {
        int r = random.next();
        return new Pos(minX + (r & 15), minY + (r >>> 16 & 15), minZ + (r >>> 8 & 15));
    }

    static void keep(Pos pos) {
        kept[keptCount++ & 63] = pos;
    }

    // One tick: three random positions in each of the sections, each read whole; the few whose
    // block ticks are kept.
    static long tick(byte[] blocks, Xorshift random, int sections) {
        long sum = 0;
        for (int s = 0; s < sections; s++) {
            int minX = (s & 7) << 4;
            int minY = (s >>> 3 & 7) << 4;
            int minZ = s >>> 6 << 4;
            for (int i = 0; i < 3; i++) {
                Pos pos = randomPos(random, minX, minY, minZ);
                int x = pos.getX() - minX;
                int y = pos.getY() - minY;
                int z = pos.getZ() - minZ;
                int block = blocks[(s << 12) | (y << 8) | (z << 4) | x];
                sum += block;
                if (block == 127) {
                    keep(pos);
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int sections = 256;
        byte[] blocks = new byte[sections << 12];
        Xorshift fill = new Xorshift();
        for (int i = 0; i < blocks.length; i++) {
            blocks[i] = (byte) (fill.next() & 127);
        }
        kept[0] = new MutablePos(0, 0, 0);
        Xorshift random = new Xorshift();
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            for (int t = 0; t < 200; t++) {
                result[0] += tick(blocks, random, sections);
            }
            long start = System.nanoTime();
            for (int t = 0; t < 20000; t++) {
                result[1] += tick(blocks, random, sections);
            }
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        long keptSum = 0;
        for (Pos pos : kept) {
            if (pos != null) {
                keptSum += pos.getX() + pos.getY() + pos.getZ();
            }
        }
        System.out.println(result[0] + " " + result[1] + " " + keptCount + " " + keptSum);
    }
}
