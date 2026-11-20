// Default methods that call other methods of their interface, on objects of many classes: a
// world and its chunks answer heights, and the default methods built on `minY` and `height` run
// on whichever. The hot calls are on a world held in a field typed by the interface, and on
// chunks, which ask the world they hold through the same interface.
public class DefaultMethodReceivers {
    interface Heights {
        int minY();

        int height();

        default int maxY() {
            return minY() + height() - 1;
        }

        default boolean inside(int y) {
            return y >= minY() && y <= maxY();
        }

        default int minSection() {
            return minY() >> 4;
        }

        default int sectionIndex(int y) {
            return (y >> 4) - minSection();
        }
    }

    static final class Dimension {
        final int minY;
        final int height;

        Dimension(int minY, int height) {
            this.minY = minY;
            this.height = height;
        }
    }

    abstract static class Level implements Heights {
        final Dimension dimension;

        Level(Dimension dimension) {
            this.dimension = dimension;
        }

        @Override
        public int minY() {
            return dimension.minY;
        }

        @Override
        public int height() {
            return dimension.height;
        }

        boolean valid(int y) {
            return inside(y);
        }
    }

    static final class ServerLevel extends Level {
        ServerLevel(Dimension dimension) {
            super(dimension);
        }
    }

    static final class ClientLevel extends Level {
        ClientLevel(Dimension dimension) {
            super(dimension);
        }
    }

    abstract static class Chunk implements Heights {
        final Heights accessor;
        final int[] blocks;

        Chunk(Heights accessor) {
            this.accessor = accessor;
            this.blocks = new int[accessor.height()];
            for (int i = 0; i < blocks.length; i++) {
                blocks[i] = i * 7 % 13;
            }
        }

        @Override
        public int minY() {
            return accessor.minY();
        }

        @Override
        public int height() {
            return accessor.height();
        }

        int block(int y) {
            return blocks[y - minY()] + sectionIndex(y);
        }
    }

    static final class LevelChunk extends Chunk {
        LevelChunk(Heights accessor) {
            super(accessor);
        }
    }

    static final class ProtoChunk extends Chunk {
        ProtoChunk(Heights accessor) {
            super(accessor);
        }
    }

    static final class ImposterChunk extends Chunk {
        ImposterChunk(Heights accessor) {
            super(accessor);
        }

        @Override
        public int minY() {
            return accessor.minY() + 0;
        }
    }

    static final class Region implements Heights {
        @Override
        public int minY() {
            return -16;
        }

        @Override
        public int height() {
            return 32;
        }
    }

    static final class Fixed implements Heights {
        final int min;

        Fixed(int min) {
            this.min = min;
        }

        @Override
        public int minY() {
            return min;
        }

        @Override
        public int height() {
            return 16;
        }
    }

    static final class Game {
        final Heights world;

        Game(Heights world) {
            this.world = world;
        }

        long count(Chunk chunk, int rounds) {
            long total = 0;
            for (int r = 0; r < rounds; r++) {
                for (int y = world.minY() - 2; y <= world.maxY() + 2; y++) {
                    if (world.inside(y)) {
                        total += chunk.block(y) + world.sectionIndex(y);
                    }
                }
            }
            return total;
        }
    }

    public static void main(String[] args) {
        Dimension overworld = new Dimension(-64, 384);
        ServerLevel level = new ServerLevel(overworld);
        Game game = new Game(level);
        LevelChunk chunk = new LevelChunk(level);
        System.out.println("level " + level.minY() + " " + level.maxY() + " " + level.valid(400));
        System.out.println("game " + game.count(chunk, 50));

        // The other classes that answer heights, each used a little.
        Heights[] others = {
            new ClientLevel(new Dimension(0, 256)),
            new ProtoChunk(level),
            new ImposterChunk(level),
            new Region(),
            new Fixed(8),
        };
        for (Heights h : others) {
            System.out.println(h.getClass().getSimpleName() + " " + h.maxY() + " " + h.inside(10) + " "
                    + h.sectionIndex(20) + " " + h.minSection());
        }
        // A second game, on a region: the world field holds a ServerLevel or a Region, two of the
        // classes, so its calls dispatch among those two and run their copies of the default
        // methods.
        Region region = new Region();
        System.out.println("proto " + new Game(region).count(new ProtoChunk(region), 2));
    }
}
