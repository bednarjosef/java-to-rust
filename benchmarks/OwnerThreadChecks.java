// The Minecraft server's chunk cache: each lookup first checks that it runs on the thread that
// owns the cache (`Thread.currentThread() != this.mainThread`), then searches a cache of the last
// four chunks asked for. The server's ticks make such lookups for every block they read near a
// chunk's edge. It runs on a worker thread, which owns the cache.
public class OwnerThreadChecks {
    static final class Chunk {
        final long pos;

        Chunk(long pos) {
            this.pos = pos;
        }
    }

    static final class ChunkCache {
        final Thread owner;
        final long[] lastPos = new long[4];
        final Chunk[] lastChunk = new Chunk[4];
        int misses;

        ChunkCache(Thread owner) {
            this.owner = owner;
            for (int i = 0; i < 4; i++) {
                lastPos[i] = Long.MIN_VALUE;
            }
        }

        Chunk get(int x, int z) {
            if (Thread.currentThread() != owner) {
                throw new IllegalStateException("not the owner");
            }
            long pos = (long) x & 0xffffffffL | ((long) z & 0xffffffffL) << 32;
            for (int i = 0; i < 4; i++) {
                if (lastPos[i] == pos) {
                    return lastChunk[i];
                }
            }
            misses++;
            for (int i = 3; i > 0; i--) {
                lastPos[i] = lastPos[i - 1];
                lastChunk[i] = lastChunk[i - 1];
            }
            Chunk chunk = new Chunk(pos);
            lastPos[0] = pos;
            lastChunk[0] = chunk;
            return chunk;
        }
    }

    static long walk(ChunkCache cache, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int x = 0; x < 64; x++) {
                for (int z = 0; z < 64; z++) {
                    sum += cache.get(x >> 4, z >> 4).pos & 0xff;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        long[] result = new long[2];
        ChunkCache[] caches = new ChunkCache[1];
        Thread worker = new Thread(() -> {
            caches[0] = new ChunkCache(Thread.currentThread());
            result[0] = walk(caches[0], 100);
            long start = System.nanoTime();
            result[1] = walk(caches[0], 10000);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println(result[0] + " " + result[1] + " " + caches[0].misses);
    }
}
