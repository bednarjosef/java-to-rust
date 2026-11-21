// A mutable position on the stack, moved over a box of blocks and passed to the section's
// lookup, as the Minecraft server's block walks use BlockPos.MutableBlockPos. The positions'
// classes are ones that threads share (the main thread's origin is read by the worker), so their
// fields are atomic cells, though the walk's position never leaves the walk.
public class MutablePositions {
    static class Vec3i {
        protected int x, y, z;

        Vec3i(int x, int y, int z) {
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

    static class BlockPos extends Vec3i {
        BlockPos(int x, int y, int z) {
            super(x, y, z);
        }

        BlockPos offset(int dx, int dy, int dz) {
            return new BlockPos(x + dx, y + dy, z + dz);
        }
    }

    static final class MutableBlockPos extends BlockPos {
        MutableBlockPos(int x, int y, int z) {
            super(x, y, z);
        }

        MutableBlockPos set(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
            return this;
        }

        MutableBlockPos move(int dx, int dy, int dz) {
            return set(x + dx, y + dy, z + dz);
        }
    }

    static final class Section {
        final short[] states = new short[4096];

        int get(BlockPos p) {
            return states[(p.getY() & 15) << 8 | (p.getZ() & 15) << 4 | (p.getX() & 15)];
        }
    }

    static BlockPos origin;

    static long walk(Section section, BlockPos from) {
        MutableBlockPos p = new MutableBlockPos(0, 0, 0);
        long sum = 0;
        for (int dy = 0; dy < 4; dy++) {
            for (int dz = 0; dz < 4; dz++) {
                p.set(from.getX(), from.getY() + dy, from.getZ() + dz);
                for (int dx = 0; dx < 4; dx++) {
                    sum += section.get(p);
                    p.move(1, 0, 0);
                }
            }
        }
        return sum;
    }

    static long run(Section[] sections, int rounds) {
        long sum = 0;
        int seed = 12345;
        for (int r = 0; r < rounds; r++) {
            for (Section s : sections) {
                seed = seed * 1103515245 + 12345;
                sum += walk(s, origin.offset(seed >>> 28, seed >>> 24 & 15, seed >>> 20 & 15));
            }
        }
        return sum;
    }

    public static void main(String[] args) throws Exception {
        origin = new BlockPos(1, 2, 3);
        Section[] sections = new Section[64];
        int seed = 7;
        for (int s = 0; s < sections.length; s++) {
            sections[s] = new Section();
            for (int i = 0; i < 4096; i++) {
                seed = seed * 1103515245 + 12345;
                sections[s].states[i] = (short) (seed >>> 20 & 255);
            }
        }
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            result[0] = run(sections, 1000);
            long start = System.nanoTime();
            result[1] = run(sections, 100000);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println("mutable positions " + result[1] + " " + result[0]);
    }
}
