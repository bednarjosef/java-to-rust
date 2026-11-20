// The Minecraft server's queries that pass lambdas to helpers: `Entity.checkInsideBlocks` hands
// `BlockGetter.forEachBlockIntersectedBetween` a visitor for each block along a movement, and
// `EntitySectionStorage.getEntities` hands `forEachAccessibleNonEmptySection` a lambda that
// captures the caller's consumer. Every query makes its lambdas again, and the helpers only call
// them.
public class LambdaVisitors {
    interface BlockVisitor { boolean visit(int x, int y, int z, int step); }
    interface Consumer { int accept(int entity); }
    interface SectionVisitor { int visit(int[] section); }

    static final int[][] SECTIONS = new int[64][];

    static boolean forEachBlockBetween(int x0, int z0, int x1, int z1, BlockVisitor visitor) {
        int step = 0;
        for (int x = Math.min(x0, x1); x <= Math.max(x0, x1); x++) {
            for (int z = Math.min(z0, z1); z <= Math.max(z0, z1); z++) {
                if (!visitor.visit(x, 64, z, step++)) return false;
            }
        }
        return true;
    }

    static int forEachSection(int from, int to, SectionVisitor visitor) {
        int found = 0;
        for (int i = from; i < to; i++) {
            int[] section = SECTIONS[i & 63];
            if (section.length > 0) found += visitor.visit(section);
        }
        return found;
    }

    static int getEntities(int from, int to, Consumer consumer) {
        return forEachSection(from, to, section -> {
            int n = 0;
            for (int e : section) n += consumer.accept(e);
            return n;
        });
    }

    public static void main(String[] args) {
        for (int i = 0; i < SECTIONS.length; i++) {
            SECTIONS[i] = new int[i % 3 == 0 ? 0 : 1 + i % 4];
            for (int j = 0; j < SECTIONS[i].length; j++) SECTIONS[i][j] = i * 7 + j;
        }
        long[] inside = new long[1];
        long hits = 0;
        long start = System.nanoTime();
        for (int tick = 0; tick < 2_000_000; tick++) {
            int x = tick % 97, z = tick % 89, limit = tick % 13;
            forEachBlockBetween(x, z, x + 1, z + 2, (bx, by, bz, step) -> {
                inside[0] += bx * 31 + by + bz;
                return step < limit;
            });
            int threshold = tick & 255;
            hits += getEntities(tick & 15, (tick & 15) + 4, e -> e > threshold ? 1 : 0);
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        System.out.println(inside[0] + " " + hits);
    }
}
