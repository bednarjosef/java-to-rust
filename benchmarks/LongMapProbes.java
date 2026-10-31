// Lookups in an open-addressing map of long keys, as the Minecraft server finds its loaded chunks
// by position (fastutil's `Long2ObjectLinkedOpenHashMap.get`): the key is mixed, masked to a slot,
// and the slots after it are probed until the key or an empty slot, reading the map's key array
// once and its mask at each slot. The map grows as it fills (its arrays and mask change), and is
// filled on one thread and read on another, as the server's are.
public class LongMapProbes {
    abstract static class AbstractLong2ObjectFunction<V> {
        protected V defRetValue;
    }

    static final class Long2ObjectOpenHashMap<V> extends AbstractLong2ObjectFunction<V> {
        protected long[] key;
        protected V[] value;
        protected int mask;
        protected boolean containsNullKey;
        protected int n;
        protected int maxFill;
        protected int size;

        @SuppressWarnings("unchecked")
        Long2ObjectOpenHashMap() {
            this.n = 16;
            this.mask = this.n - 1;
            this.maxFill = this.n * 3 / 4;
            this.key = new long[this.n + 1];
            this.value = (V[]) new Object[this.n + 1];
        }

        static long mix(long x) {
            long h = x * -7046029254386353131L;
            h ^= h >>> 32;
            return h ^ h >>> 16;
        }

        public V get(long k) {
            if (k == 0) {
                return this.containsNullKey ? this.value[this.n] : this.defRetValue;
            }
            long curr;
            final long[] key = this.key;
            int pos;
            if ((curr = key[pos = (int) mix(k) & this.mask]) == 0) {
                return this.defRetValue;
            }
            if (k == curr) {
                return this.value[pos];
            }
            while (true) {
                if ((curr = key[pos = pos + 1 & this.mask]) == 0) {
                    return this.defRetValue;
                }
                if (k == curr) {
                    return this.value[pos];
                }
            }
        }

        public void put(long k, V v) {
            if (k == 0) {
                this.containsNullKey = true;
                this.value[this.n] = v;
                return;
            }
            int pos = (int) mix(k) & this.mask;
            while (this.key[pos] != 0) {
                if (this.key[pos] == k) {
                    this.value[pos] = v;
                    return;
                }
                pos = pos + 1 & this.mask;
            }
            this.key[pos] = k;
            this.value[pos] = v;
            if (++this.size >= this.maxFill) {
                this.rehash(this.n * 2);
            }
        }

        @SuppressWarnings("unchecked")
        private void rehash(int newN) {
            long[] oldKey = this.key;
            V[] oldValue = this.value;
            int mask = newN - 1;
            long[] newKey = new long[newN + 1];
            V[] newValue = (V[]) new Object[newN + 1];
            for (int i = 0; i < this.n; i++) {
                if (oldKey[i] != 0) {
                    int pos = (int) mix(oldKey[i]) & mask;
                    while (newKey[pos] != 0) {
                        pos = pos + 1 & mask;
                    }
                    newKey[pos] = oldKey[i];
                    newValue[pos] = oldValue[i];
                }
            }
            newValue[newN] = oldValue[this.n];
            this.n = newN;
            this.mask = mask;
            this.maxFill = newN * 3 / 4;
            this.key = newKey;
            this.value = newValue;
        }
    }

    static final class Chunk {
        final int x;
        final int z;

        Chunk(int x, int z) {
            this.x = x;
            this.z = z;
        }
    }

    static long asLong(int x, int z) {
        return x & 0xFFFFFFFFL | (z & 0xFFFFFFFFL) << 32;
    }

    static long run(Long2ObjectOpenHashMap<Chunk> chunks, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int x = -20; x <= 20; x++) {
                for (int z = -20; z <= 20; z++) {
                    Chunk chunk = chunks.get(asLong(x, z));
                    sum += chunk == null ? 1 : chunk.x - chunk.z;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 6000;
        Long2ObjectOpenHashMap<Chunk> chunks = new Long2ObjectOpenHashMap<>();
        for (int x = -16; x <= 16; x++) {
            for (int z = -16; z <= 16; z++) {
                chunks.put(asLong(x, z), new Chunk(x, z));
            }
        }
        long[] result = new long[2];
        Thread reader = new Thread(() -> {
            result[0] = run(chunks, 100);
            long start = System.nanoTime();
            result[1] = run(chunks, rounds);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        reader.start();
        reader.join();
        System.out.println("long map probes " + result[0] + " " + result[1]);
    }
}
