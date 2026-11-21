// The Minecraft server's block updates look at a position's neighbours: `pos.relative(d)` makes a
// position from the direction's three steps, fields of an enum whose static initialization builds
// its constants (so each step's method checks that the enum is initialized), and the chunk of a
// position comes from two calls of a static helper of a class with a static initializer
// (`SectionPos.blockToSectionCoord`) in one call's arguments. It runs on a worker thread, as the
// server's ticks do, so each check is a load of the process's flag.
public class NeighborSteps {
    enum Axis {
        X,
        Y,
        Z
    }

    enum Direction {
        DOWN(Axis.Y, 0, -1, 0),
        UP(Axis.Y, 0, 1, 0),
        NORTH(Axis.Z, 0, 0, -1),
        SOUTH(Axis.Z, 0, 0, 1),
        WEST(Axis.X, -1, 0, 0),
        EAST(Axis.X, 1, 0, 0);

        static final Direction[] VALUES = values();
        final Axis axis;
        final int stepX;
        final int stepY;
        final int stepZ;

        Direction(Axis axis, int stepX, int stepY, int stepZ) {
            this.axis = axis;
            this.stepX = stepX;
            this.stepY = stepY;
            this.stepZ = stepZ;
        }

        int getStepX() {
            return stepX;
        }

        int getStepY() {
            return stepY;
        }

        int getStepZ() {
            return stepZ;
        }
    }

    static final class Sections {
        static final int SHIFT = shiftOf(16);

        static int shiftOf(int size) {
            int shift = 0;
            while ((1 << shift) < size) {
                shift++;
            }
            return shift;
        }

        static int blockToSection(int block) {
            return block >> SHIFT;
        }

        static long asLong(int x, int z) {
            return ((long) x << 32) | (z & 0xffffffffL);
        }
    }

    static final class Pos {
        final int x;
        final int y;
        final int z;

        Pos(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        Pos relative(Direction d) {
            return new Pos(x + d.getStepX(), y + d.getStepY(), z + d.getStepZ());
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

    static long chunkKey(Pos pos) {
        return Sections.asLong(Sections.blockToSection(pos.x), Sections.blockToSection(pos.z));
    }

    // One tick: the six neighbours of each of 4096 random positions, and the chunk of each.
    static long tick(Xorshift random) {
        long sum = 0;
        for (int i = 0; i < 4096; i++) {
            int r = random.next();
            Pos pos = new Pos(r & 255, r >>> 8 & 127, r >>> 16 & 255);
            for (Direction d : Direction.VALUES) {
                Pos next = pos.relative(d);
                sum += chunkKey(next) ^ next.y;
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        Xorshift random = new Xorshift();
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            for (int t = 0; t < 20; t++) {
                result[0] += tick(random);
            }
            long start = System.nanoTime();
            for (int t = 0; t < 2000; t++) {
                result[1] += tick(random);
            }
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println(result[0] + " " + result[1]);
    }
}
