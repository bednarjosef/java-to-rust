import java.util.ArrayList;
import java.util.List;

// A game server's tick, as the Minecraft server's: the loop and each entity's tick run inside a
// catch of Throwable that turns a failure into a crash report, far from the small methods that
// may throw (a validation helper, an array index, a null field), which never do here. Each
// entity reads blocks from a packed bit storage and moves.
public class GuardedTicks {
    static final class Storage {
        final long[] data;
        final int bits;
        final int size;
        final int valuesPerLong;
        final long mask;

        Storage(int bits, int size) {
            this.bits = bits;
            this.size = size;
            this.valuesPerLong = 64 / bits;
            this.mask = (1L << bits) - 1;
            this.data = new long[(size + valuesPerLong - 1) / valuesPerLong];
        }

        int get(int index) {
            inclusiveBetween(0, size - 1, index);
            int cell = index / valuesPerLong;
            int shift = (index - cell * valuesPerLong) * bits;
            return (int) (data[cell] >> shift & mask);
        }

        void set(int index, int value) {
            inclusiveBetween(0, size - 1, index);
            int cell = index / valuesPerLong;
            int shift = (index - cell * valuesPerLong) * bits;
            data[cell] = data[cell] & ~(mask << shift) | ((long) value & mask) << shift;
        }
    }

    static void inclusiveBetween(long start, long end, long value) {
        if (value < start || value > end) {
            throw new IllegalArgumentException("The value " + value + " is not in the range " + start + " to " + end);
        }
    }

    static final class Section {
        final Storage states = new Storage(5, 4096);
        Section below;

        int blockAt(int x, int y, int z) {
            return states.get((y & 15) << 8 | (z & 15) << 4 | (x & 15));
        }
    }

    static final class Level {
        final Section[] sections = new Section[16];
        final List<Entity> entities = new ArrayList<>();
        long seed = 42;

        Level() {
            for (int i = 0; i < sections.length; i++) {
                sections[i] = new Section();
                sections[i].below = i > 0 ? sections[i - 1] : null;
                for (int b = 0; b < 4096; b++) {
                    sections[i].states.set(b, (b * 7 + i) & 31);
                }
            }
        }

        int next(int bound) {
            seed = seed * 6364136223846793005L + 1442695040888963407L;
            return (int) ((seed >>> 33) % bound);
        }

        int blockAt(int x, int y, int z) {
            return sections[(y >> 4) & 15].blockAt(x, y, z);
        }

        void tick() {
            for (Entity e : entities) {
                guardEntityTick(e);
            }
            for (int i = 0; i < 64; i++) {
                int y = next(256);
                Section s = sections[y >> 4];
                if (s.below != null && s.blockAt(next(16), y, next(16)) == 3) {
                    s.below.states.set(next(4096), 4);
                }
            }
        }

        void guardEntityTick(Entity e) {
            try {
                e.tick(this);
            } catch (Throwable t) {
                throw new RuntimeException("Ticking entity " + e.id, t);
            }
        }
    }

    static class Entity {
        final int id;
        int x;
        int y;
        int z;
        int seen;

        Entity(int id) {
            this.id = id;
            this.x = id * 13;
            this.y = id * 7 & 255;
            this.z = id * 3;
        }

        void tick(Level level) {
            for (int dy = -2; dy <= 2; dy++) {
                seen += level.blockAt(x, y + dy & 255, z);
            }
            x += level.next(3) - 1;
            z += level.next(3) - 1;
        }
    }

    static final class Server {
        final Level level = new Level();
        long ticks;

        Server() {
            for (int i = 0; i < 400; i++) {
                level.entities.add(new Entity(i));
            }
        }

        void run(int count) {
            try {
                for (int i = 0; i < count; i++) {
                    level.tick();
                    ticks++;
                }
            } catch (Throwable t) {
                System.out.println("crash: " + t);
            }
        }
    }

    public static void main(String[] args) {
        Server server = new Server();
        server.run(2000);
        long start = System.nanoTime();
        server.run(20000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        long sum = 0;
        for (Entity e : server.level.entities) {
            sum += e.seen;
        }
        System.out.println("guarded ticks " + server.ticks + " " + sum);
        System.err.println("kernel-ms " + elapsed);
    }
}
