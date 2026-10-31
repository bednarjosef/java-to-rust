// Random numbers from sources behind one interface, as the Minecraft server draws them on every
// tick of every chunk (`BitRandomSource.next`): each source steps a 48-bit linear congruential
// seed, `LegacyRandomSource` in an `AtomicLong` that it sets with a compare-and-set (and throws
// where another thread changed it meanwhile), `SingleThreadedRandomSource` in a plain field, and
// `nextInt(bound)` takes its bits from `next` through the interface. The sources are made on one
// thread and drawn from on another, as the server's are.
import java.util.concurrent.atomic.AtomicLong;

public class RandomBits {
    interface RandomSource {
        int nextInt(int bound);
    }

    interface BitRandomSource extends RandomSource {
        int next(int bits);

        default int nextInt(int bound) {
            if (bound <= 0) {
                throw new IllegalArgumentException("Bound must be positive");
            }
            if ((bound & bound - 1) == 0) {
                return (int) ((long) bound * this.next(31) >> 31);
            }
            int sample;
            int modulo;
            do {
                sample = this.next(31);
                modulo = sample % bound;
            } while (sample - modulo + (bound - 1) < 0);
            return modulo;
        }
    }

    static final class LegacyRandomSource implements BitRandomSource {
        private final AtomicLong seed = new AtomicLong();

        LegacyRandomSource(long seed) {
            if (!this.seed.compareAndSet(this.seed.get(), (seed ^ 25214903917L) & 281474976710655L)) {
                throw new IllegalStateException("LegacyRandomSource is accessed from multiple threads");
            }
        }

        public int next(int bits) {
            long oldSeed = this.seed.get();
            long newSeed = oldSeed * 25214903917L + 11L & 281474976710655L;
            if (!this.seed.compareAndSet(oldSeed, newSeed)) {
                throw new IllegalStateException("LegacyRandomSource is accessed from multiple threads");
            }
            return (int) (newSeed >> 48 - bits);
        }
    }

    static final class SingleThreadedRandomSource implements BitRandomSource {
        private long seed;

        SingleThreadedRandomSource(long seed) {
            this.seed = (seed ^ 25214903917L) & 281474976710655L;
        }

        public int next(int bits) {
            long newSeed = this.seed * 25214903917L + 11L & 281474976710655L;
            this.seed = newSeed;
            return (int) (newSeed >> 48 - bits);
        }
    }

    static final class WorldgenRandom implements BitRandomSource {
        private final RandomSource randomSource;
        private int count;

        WorldgenRandom(RandomSource randomSource) {
            this.randomSource = randomSource;
        }

        public int next(int bits) {
            this.count++;
            return this.randomSource instanceof BitRandomSource bits0 ? bits0.next(bits) : this.randomSource.nextInt(1 << bits);
        }
    }

    static long run(RandomSource[] sources, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (RandomSource source : sources) {
                for (int i = 0; i < 64; i++) {
                    sum += source.nextInt(16) + source.nextInt(48);
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 20000;
        RandomSource[] sources = new RandomSource[16];
        for (int i = 0; i < sources.length; i++) {
            sources[i] = i == 5 ? new SingleThreadedRandomSource(i) : i == 11 ? new WorldgenRandom(new LegacyRandomSource(i)) : new LegacyRandomSource(i);
        }
        long[] result = new long[2];
        Thread drawer = new Thread(() -> {
            result[0] = run(sources, 200);
            long start = System.nanoTime();
            result[1] = run(sources, rounds);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        drawer.start();
        drawer.join();
        System.out.println("random bits " + result[0] + " " + result[1]);
    }
}
