// An object kept only on a rare branch starts on the stack and moves to the heap there, though a
// call that computes its arguments first (its receiver may be null and a later argument has an
// effect) copies it into a temporary of its own: that temporary is not read after the move.
public class StackObjectsMovedPastArgumentCopies {
    static final class Pos {
        final int x, y, z;
        Pos(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        Pos below() { return new Pos(x, y - 1, z); }
    }

    static final class Biome {
        final int wetness;
        Biome(int wetness) { this.wetness = wetness; }
        int precipitationAt(Pos p, int seaLevel) {
            return p.y < seaLevel ? 0 : (p.x + p.z + wetness) & 3;
        }
    }

    static final Biome[] biomes = { new Biome(0), new Biome(1), new Biome(2), null };
    static final Pos[] kept = new Pos[64];
    static int count;
    static int seaLevelReads;

    static int seaLevel() {
        seaLevelReads++;
        return 62;
    }

    static Biome biomeAt(Pos p) {
        return biomes[(p.x ^ p.z) % 3];
    }

    static void keep(Pos p) { kept[count++] = p; }

    public static void main(String[] args) {
        int sum = 0;
        for (int i = 0; i < 200; i++) {
            Pos top = new Pos(i % 17, 60 + i % 9, i % 13);
            Pos below = top.below();
            Biome biome = biomeAt(top);
            int precipitation = biome.precipitationAt(below, seaLevel());
            sum += precipitation;
            if (precipitation == 3 && count < kept.length) {
                keep(below);
            }
        }
        System.out.println(sum + " " + count + " " + seaLevelReads);
        System.out.println(kept[0].x + " " + kept[0].y + " " + kept[0].z + " " + kept[count - 1].y);
    }
}
