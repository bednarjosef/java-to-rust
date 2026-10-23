// Compression with java.util.zip.Deflater, read back with Inflater. The translation's compressor
// is its own, so the compressed bytes differ from zlib's; what is printed is what both give: the
// text read back, the bytes read, the Adler-32 checksums, the flags, the zlib header, the flush
// markers, and the failures.
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.zip.DataFormatException;
import java.util.zip.Deflater;
import java.util.zip.DeflaterOutputStream;
import java.util.zip.GZIPInputStream;
import java.util.zip.GZIPOutputStream;
import java.util.zip.Inflater;
import java.util.zip.InflaterInputStream;

public class DeflaterRoundTrips {
    static byte[] text(int words) {
        String[] parts = {"deflate ", "inflate ", "stream ", "window ", "the ", "block\n", "zlib "};
        StringBuilder b = new StringBuilder();
        int seed = 7;
        for (int i = 0; i < words; i++) {
            seed = seed * 1103515245 + 12345;
            int pick = (seed >>> 16) % 23;
            b.append(pick < parts.length ? parts[pick] : Integer.toString(pick * 37));
        }
        return b.toString().getBytes(StandardCharsets.UTF_8);
    }

    /** Compresses all of `data` into pieces of `room` bytes. */
    static byte[] compress(Deflater d, byte[] data, int room) {
        d.setInput(data);
        d.finish();
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[room];
        while (!d.finished()) {
            int n = d.deflate(buf);
            out.write(buf, 0, n);
        }
        return out.toByteArray();
    }

    static byte[] inflate(byte[] z, boolean nowrap, int size) throws DataFormatException {
        Inflater inf = new Inflater(nowrap);
        inf.setInput(z);
        byte[] out = new byte[size];
        int n = inf.inflate(out);
        System.out.println("  inflated " + n + " finished " + inf.finished() + " remaining " + inf.getRemaining()
                + " adler " + Integer.toHexString(inf.getAdler()));
        inf.end();
        return Arrays.copyOf(out, n);
    }

    static void levels(byte[] data) throws DataFormatException {
        for (int level = -1; level <= 9; level++) {
            for (boolean nowrap : new boolean[] {false, true}) {
                Deflater d = new Deflater(level, nowrap);
                byte[] z = compress(d, data, level == 3 ? 7 : 4096);
                System.out.println("level " + level + " nowrap " + nowrap + ": read " + d.getTotalIn() + " "
                        + d.getBytesRead() + " adler " + Integer.toHexString(d.getAdler()) + " smaller "
                        + (z.length < data.length) + " header " + (nowrap ? "-" : Integer.toHexString(z[0] & 0xff)
                        + " " + Integer.toHexString(z[1] & 0xff)) + " out counted " + (d.getBytesWritten() == z.length));
                byte[] back = inflate(z, nowrap, data.length + 10);
                System.out.println("  same " + Arrays.equals(back, data));
                d.end();
            }
        }
    }

    static void empty() {
        Deflater d = new Deflater();
        byte[] z = compress(d, new byte[0], 64);
        // zlib's bytes for no input, which the translation writes too.
        System.out.println("empty: " + Arrays.toString(z));
        d.reset();
        System.out.println("after reset: finished " + d.finished() + " needs input " + d.needsInput()
                + " read " + d.getBytesRead() + " adler " + d.getAdler());
        System.out.println("again: " + Arrays.toString(compress(d, new byte[0], 64)));
        d.end();
    }

    static void flushes(byte[] data) throws DataFormatException {
        for (int mode : new int[] {Deflater.SYNC_FLUSH, Deflater.FULL_FLUSH}) {
            Deflater d = new Deflater();
            d.setInput(data, 0, 1000);
            byte[] buf = new byte[data.length + 100];
            int n = d.deflate(buf, 0, buf.length, mode);
            System.out.println("flush " + mode + ": needs input " + d.needsInput() + " marker "
                    + Arrays.toString(Arrays.copyOfRange(buf, n - 4, n)) + " again " + d.deflate(buf, n, 100, mode));
            // What is flushed reads back without the rest.
            Inflater partial = new Inflater();
            partial.setInput(buf, 0, n);
            byte[] first = new byte[2000];
            int got = partial.inflate(first);
            System.out.println("  partial " + got + " " + Arrays.equals(Arrays.copyOf(first, got), Arrays.copyOf(data, 1000)));
            d.setInput(data, 1000, data.length - 1000);
            d.finish();
            while (!d.finished()) {
                n += d.deflate(buf, n, buf.length - n);
            }
            System.out.println("  same " + Arrays.equals(inflate(Arrays.copyOf(buf, n), false, data.length), data));
            d.end();
        }
    }

    static void dictionary(byte[] data) throws DataFormatException {
        byte[] dict = "deflate inflate stream window the block zlib".getBytes(StandardCharsets.UTF_8);
        Deflater d = new Deflater(Deflater.BEST_COMPRESSION);
        d.setDictionary(dict);
        System.out.println("dictionary adler " + Integer.toHexString(d.getAdler()));
        byte[] z = compress(d, data, 512);
        System.out.println("  header flags " + Integer.toHexString(z[1] & 0xff));
        // zlib takes a dictionary once the stream has ended, and not while it is under way.
        d.setDictionary(dict);
        System.out.println("  after the end: adler " + Integer.toHexString(d.getAdler()));
        Deflater begun = new Deflater();
        begun.setInput(data, 0, 10);
        begun.deflate(new byte[100], 0, 100, Deflater.SYNC_FLUSH);
        try {
            begun.setDictionary(dict);
        } catch (IllegalArgumentException e) {
            System.out.println("  under way: " + e);
        }
        begun.end();
        Inflater inf = new Inflater();
        inf.setInput(z);
        byte[] out = new byte[data.length];
        int n = inf.inflate(out);
        System.out.println("  needs dictionary " + inf.needsDictionary() + " " + n + " adler "
                + Integer.toHexString(inf.getAdler()));
        inf.setDictionary(dict);
        n = inf.inflate(out);
        System.out.println("  same " + (n == data.length && Arrays.equals(out, data)));
        d.end();
        inf.end();
    }

    static void params(byte[] data) throws DataFormatException {
        Deflater d = new Deflater(Deflater.BEST_SPEED);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[300];
        int third = data.length / 3;
        d.setInput(data, 0, third);
        while (!d.needsInput()) {
            out.write(buf, 0, d.deflate(buf));
        }
        d.setLevel(Deflater.NO_COMPRESSION);
        d.setInput(data, third, third);
        while (!d.needsInput()) {
            out.write(buf, 0, d.deflate(buf));
        }
        d.setStrategy(Deflater.HUFFMAN_ONLY);
        d.setLevel(Deflater.DEFAULT_COMPRESSION);
        d.setInput(data, 2 * third, data.length - 2 * third);
        d.finish();
        while (!d.finished()) {
            out.write(buf, 0, d.deflate(buf));
        }
        System.out.println("params: read " + d.getBytesRead() + " adler " + Integer.toHexString(d.getAdler()));
        System.out.println("  same " + Arrays.equals(inflate(out.toByteArray(), false, data.length), data));
        d.end();
    }

    static void buffers(byte[] data) throws DataFormatException {
        ByteBuffer input = ByteBuffer.allocateDirect(data.length);
        input.put(data).flip();
        ByteBuffer output = ByteBuffer.allocateDirect(data.length + 64);
        Deflater d = new Deflater();
        d.setInput(input);
        d.finish();
        while (!d.finished()) {
            d.deflate(output);
        }
        output.flip();
        byte[] z = new byte[output.remaining()];
        output.get(z);
        System.out.println("direct buffers: input left " + input.remaining() + " read " + d.getBytesRead());
        System.out.println("  same " + Arrays.equals(inflate(z, false, data.length), data));
        ByteBuffer heap = ByteBuffer.wrap(data, 10, 500);
        d.reset();
        d.setInput(heap);
        d.finish();
        ByteBuffer heapOut = ByteBuffer.allocate(1000);
        while (!d.finished()) {
            d.deflate(heapOut, Deflater.NO_FLUSH);
        }
        System.out.println("heap buffers: input left " + heap.remaining() + " out position counted "
                + (heapOut.position() == d.getBytesWritten()));
        d.end();
    }

    static void streams(byte[] data) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DeflaterOutputStream out = new DeflaterOutputStream(bytes, true)) {
            out.write(data, 0, 100);
            out.flush();
            System.out.println("streams: flushed reads back "
                    + new String(new InflaterInputStream(new ByteArrayInputStream(bytes.toByteArray())).readNBytes(100),
                            StandardCharsets.UTF_8).equals(new String(data, 0, 100, StandardCharsets.UTF_8)));
            out.write(data, 100, data.length - 100);
        }
        byte[] back = new InflaterInputStream(new ByteArrayInputStream(bytes.toByteArray())).readAllBytes();
        System.out.println("  same " + Arrays.equals(back, data));
        ByteArrayOutputStream gz = new ByteArrayOutputStream();
        try (GZIPOutputStream out = new GZIPOutputStream(gz)) {
            out.write(data);
        }
        byte[] g = gz.toByteArray();
        System.out.println("gzip: magic " + Integer.toHexString(g[0] & 0xff) + Integer.toHexString(g[1] & 0xff)
                + " same " + Arrays.equals(new GZIPInputStream(new ByteArrayInputStream(g)).readAllBytes(), data));
    }

    static void failures() {
        try {
            new Deflater(10);
        } catch (IllegalArgumentException e) {
            System.out.println("level 10: " + e);
        }
        Deflater d = new Deflater();
        try {
            d.setLevel(11);
        } catch (IllegalArgumentException e) {
            System.out.println("setLevel 11: " + e.getMessage());
        }
        try {
            d.deflate(new byte[4], 0, 4, 7);
        } catch (IllegalArgumentException e) {
            System.out.println("flush 7: " + e);
        }
        try {
            d.deflate(new byte[4], 2, 4);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("out of bounds: " + e.getMessage());
        }
        try {
            d.deflate((byte[]) null);
        } catch (NullPointerException e) {
            System.out.println("null output: " + e.getMessage());
        }
        d.end();
        try {
            d.deflate(new byte[4]);
        } catch (IllegalStateException e) {
            System.out.println("after end: " + e.getMessage());
        }
    }

    public static void main(String[] args) throws Exception {
        byte[] data = text(20000);
        System.out.println("input " + data.length);
        levels(data);
        empty();
        flushes(data);
        dictionary(data);
        params(data);
        buffers(data);
        streams(data);
        failures();
    }
}
