// World generation's shape: a tree of density functions behind one interface, each filling a
// buffer of a chunk's cells from its children's buffers, with Perlin noise at the leaves (a
// permutation table, a static gradient table and static helpers for lerp, floor and smoothstep).
// Most calls are tiny: a buffer's get and set, a noise octave's gradient, a static helper. It
// runs on a worker thread, as the server's generation does.
public class NoiseTree {
    static final class Mth {
        static int floor(double v) {
            int i = (int) v;
            return v < i ? i - 1 : i;
        }

        static double lerp(double a, double from, double to) {
            return from + a * (to - from);
        }

        static double lerp2(double a1, double a2, double x00, double x10, double x01, double x11) {
            return lerp(a2, lerp(a1, x00, x10), lerp(a1, x01, x11));
        }

        static double lerp3(double a1, double a2, double a3, double x000, double x100, double x010,
                double x110, double x001, double x101, double x011, double x111) {
            return lerp(a3, lerp2(a1, a2, x000, x100, x010, x110), lerp2(a1, a2, x001, x101, x011, x111));
        }

        static double smoothstep(double x) {
            return x * x * x * (x * (x * 6.0 - 15.0) + 10.0);
        }

        static double clamp(double v, double min, double max) {
            return v < min ? min : Math.min(v, max);
        }
    }

    static class Buffer {
        protected final float[] values;
        protected int size;

        Buffer(int size) {
            this.values = new float[size];
            this.size = size;
        }

        float get(int i) {
            return values[i];
        }

        void set(int i, float v) {
            values[i] = v;
        }

        void addTo(int i, float v) {
            values[i] = values[i] + v;
        }

        int size() {
            return size;
        }
    }

    static final class ScopedBuffer extends Buffer {
        int depth;

        ScopedBuffer(int size, int depth) {
            super(size);
            this.depth = depth;
        }
    }

    static final class Gradient {
        final double x, y, z;

        Gradient(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        double dot(double dx, double dy, double dz) {
            return x * dx + y * dy + z * dz;
        }
    }

    static final class ImprovedNoise {
        static final Gradient[] GRADIENT = {
            new Gradient(1, 1, 0), new Gradient(-1, 1, 0), new Gradient(1, -1, 0), new Gradient(-1, -1, 0),
            new Gradient(1, 0, 1), new Gradient(-1, 0, 1), new Gradient(1, 0, -1), new Gradient(-1, 0, -1),
            new Gradient(0, 1, 1), new Gradient(0, -1, 1), new Gradient(0, 1, -1), new Gradient(0, -1, -1),
            new Gradient(1, 1, 0), new Gradient(0, -1, 1), new Gradient(-1, 1, 0), new Gradient(0, -1, -1),
        };

        private final byte[] p = new byte[256];
        final double xo, yo, zo;

        ImprovedNoise(long seed) {
            long s = seed;
            for (int i = 0; i < 256; i++) {
                p[i] = (byte) i;
            }
            for (int i = 0; i < 256; i++) {
                s = s * 6364136223846793005L + 1442695040888963407L;
                int j = i + (int) ((s >>> 33) % (256 - i));
                byte t = p[i];
                p[i] = p[j];
                p[j] = t;
            }
            xo = (s >>> 11) * 0x1.0p-53 * 256;
            yo = (s >>> 13) * 0x1.0p-51 * 256;
            zo = (s >>> 17) * 0x1.0p-47 * 256;
        }

        private int permute(int x) {
            return p[x & 255] & 255;
        }

        private static double gradDot(int hash, double x, double y, double z) {
            return GRADIENT[hash & 15].dot(x, y, z);
        }

        double noise(double x, double y, double z) {
            double px = x + xo, py = y + yo, pz = z + zo;
            int ix = Mth.floor(px), iy = Mth.floor(py), iz = Mth.floor(pz);
            double fx = px - ix, fy = py - iy, fz = pz - iz;
            int a = permute(ix), b = permute(ix + 1);
            int aa = permute(a + iy), ab = permute(a + iy + 1);
            int ba = permute(b + iy), bb = permute(b + iy + 1);
            double d000 = gradDot(permute(aa + iz), fx, fy, fz);
            double d100 = gradDot(permute(ba + iz), fx - 1, fy, fz);
            double d010 = gradDot(permute(ab + iz), fx, fy - 1, fz);
            double d110 = gradDot(permute(bb + iz), fx - 1, fy - 1, fz);
            double d001 = gradDot(permute(aa + iz + 1), fx, fy, fz - 1);
            double d101 = gradDot(permute(ba + iz + 1), fx - 1, fy, fz - 1);
            double d011 = gradDot(permute(ab + iz + 1), fx, fy - 1, fz - 1);
            double d111 = gradDot(permute(bb + iz + 1), fx - 1, fy - 1, fz - 1);
            return Mth.lerp3(Mth.smoothstep(fx), Mth.smoothstep(fy), Mth.smoothstep(fz),
                d000, d100, d010, d110, d001, d101, d011, d111);
        }
    }

    static final class PerlinNoise {
        private final ImprovedNoise[] octaves;

        PerlinNoise(long seed, int count) {
            octaves = new ImprovedNoise[count];
            for (int i = 0; i < count; i++) {
                octaves[i] = new ImprovedNoise(seed * 31 + i);
            }
        }

        double getValue(double x, double y, double z) {
            double value = 0, frequency = 1, amplitude = 1;
            for (ImprovedNoise octave : octaves) {
                value += amplitude * octave.noise(x * frequency, y * frequency, z * frequency);
                frequency *= 2;
                amplitude /= 2;
            }
            return value;
        }
    }

    // The cells of one chunk section: WIDTH x HEIGHT x WIDTH, the y index fastest.
    static final int WIDTH = 4, HEIGHT = 48, CELLS = WIDTH * HEIGHT * WIDTH;

    interface DensityFunction {
        double compute(int x, int y, int z);

        // Fills `out` with the values at the cells of the chunk at (cx, cz).
        default void fill(Buffer out, int cx, int cz) {
            int i = 0;
            for (int x = 0; x < WIDTH; x++) {
                for (int z = 0; z < WIDTH; z++) {
                    for (int y = 0; y < HEIGHT; y++) {
                        out.set(i++, (float) compute(cx * WIDTH + x, y, cz * WIDTH + z));
                    }
                }
            }
        }
    }

    static final class Constant implements DensityFunction {
        final double value;

        Constant(double value) {
            this.value = value;
        }

        public double compute(int x, int y, int z) {
            return value;
        }

        public void fill(Buffer out, int cx, int cz) {
            for (int i = 0; i < out.size(); i++) {
                out.set(i, (float) value);
            }
        }
    }

    static final class Noise implements DensityFunction {
        final PerlinNoise noise;
        final double xzScale, yScale;

        Noise(PerlinNoise noise, double xzScale, double yScale) {
            this.noise = noise;
            this.xzScale = xzScale;
            this.yScale = yScale;
        }

        public double compute(int x, int y, int z) {
            return noise.getValue(x * xzScale, y * yScale, z * xzScale);
        }
    }

    static final class YGradient implements DensityFunction {
        final double from, to;

        YGradient(double from, double to) {
            this.from = from;
            this.to = to;
        }

        public double compute(int x, int y, int z) {
            return Mth.lerp(y / (double) HEIGHT, from, to);
        }
    }

    abstract static class Binary implements DensityFunction {
        final DensityFunction a, b;
        final Buffer scratch = new ScopedBuffer(CELLS, 1);

        Binary(DensityFunction a, DensityFunction b) {
            this.a = a;
            this.b = b;
        }

        abstract double apply(double x, double y);

        public double compute(int x, int y, int z) {
            return apply(a.compute(x, y, z), b.compute(x, y, z));
        }

        public void fill(Buffer out, int cx, int cz) {
            a.fill(out, cx, cz);
            b.fill(scratch, cx, cz);
            for (int i = 0; i < out.size(); i++) {
                out.set(i, (float) apply(out.get(i), scratch.get(i)));
            }
        }
    }

    static final class Add extends Binary {
        Add(DensityFunction a, DensityFunction b) {
            super(a, b);
        }

        double apply(double x, double y) {
            return x + y;
        }
    }

    static final class Mul extends Binary {
        Mul(DensityFunction a, DensityFunction b) {
            super(a, b);
        }

        double apply(double x, double y) {
            return x * y;
        }
    }

    abstract static class Unary implements DensityFunction {
        final DensityFunction input;

        Unary(DensityFunction input) {
            this.input = input;
        }

        abstract double transform(double v);

        public double compute(int x, int y, int z) {
            return transform(input.compute(x, y, z));
        }

        public void fill(Buffer out, int cx, int cz) {
            input.fill(out, cx, cz);
            for (int i = 0; i < out.size(); i++) {
                out.set(i, (float) transform(out.get(i)));
            }
        }
    }

    static final class Abs extends Unary {
        Abs(DensityFunction input) {
            super(input);
        }

        double transform(double v) {
            return Math.abs(v);
        }
    }

    static final class Square extends Unary {
        Square(DensityFunction input) {
            super(input);
        }

        double transform(double v) {
            return v * v;
        }
    }

    static final class Clamp extends Unary {
        final double min, max;

        Clamp(DensityFunction input, double min, double max) {
            super(input);
            this.min = min;
            this.max = max;
        }

        double transform(double v) {
            return Mth.clamp(v, min, max);
        }
    }

    // A function whose value at a cell is interpolated from its input at the corners of the
    // cell's column, as the noise router's interpolated functions are.
    static final class Interpolated implements DensityFunction {
        final DensityFunction input;
        final Buffer corners = new ScopedBuffer(2 * HEIGHT, 2);

        Interpolated(DensityFunction input) {
            this.input = input;
        }

        public double compute(int x, int y, int z) {
            return input.compute(x, y, z);
        }

        public void fill(Buffer out, int cx, int cz) {
            for (int y = 0; y < HEIGHT; y++) {
                corners.set(y, (float) input.compute(cx * WIDTH, y, cz * WIDTH));
                corners.set(HEIGHT + y, (float) input.compute(cx * WIDTH + WIDTH, y, cz * WIDTH + WIDTH));
            }
            int i = 0;
            for (int x = 0; x < WIDTH; x++) {
                for (int z = 0; z < WIDTH; z++) {
                    double t = (x + z) / (2.0 * WIDTH);
                    for (int y = 0; y < HEIGHT; y++) {
                        out.addTo(i, (float) Mth.lerp(t, corners.get(y), corners.get(HEIGHT + y)));
                        i++;
                    }
                }
            }
        }
    }

    static DensityFunction router(long seed) {
        DensityFunction continents = new Noise(new PerlinNoise(seed, 4), 0.01, 0.0);
        DensityFunction erosion = new Noise(new PerlinNoise(seed + 1, 3), 0.02, 0.0);
        DensityFunction ridges = new Abs(new Noise(new PerlinNoise(seed + 2, 3), 0.03, 0.01));
        DensityFunction caves = new Square(new Noise(new PerlinNoise(seed + 3, 2), 0.08, 0.12));
        DensityFunction terrain = new Add(new YGradient(1.5, -1.5),
            new Mul(new Add(continents, new Constant(0.1)), new Add(erosion, ridges)));
        DensityFunction detail = new Interpolated(new Noise(new PerlinNoise(seed + 4, 2), 0.25, 0.125));
        return new Clamp(new Add(new Add(terrain, caves), detail), -1.0, 1.0);
    }

    static double generate(DensityFunction router, int chunks) {
        Buffer out = new ScopedBuffer(CELLS, 0);
        double sum = 0;
        for (int c = 0; c < chunks; c++) {
            for (int i = 0; i < out.size(); i++) {
                out.set(i, 0f);
            }
            router.fill(out, c % 64, c / 64);
            for (int i = 0; i < out.size(); i++) {
                sum += out.get(i) > 0 ? out.get(i) : 0;
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        DensityFunction router = router(12345);
        double[] result = new double[2];
        Thread worker = new Thread(() -> {
            result[0] = generate(router, 200);
            long start = System.nanoTime();
            result[1] = generate(router, 2000);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        System.out.println((long) (result[0] * 1000) + " " + (long) (result[1] * 1000));
    }
}
