// Adler32 and CRC32 of bytes from arrays, one at a time, and from direct buffers, whose memory
// the runtime reads by address.
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.zip.Adler32;
import java.util.zip.CRC32;
import java.util.zip.Checksum;

public class Checksums {
    static void show(String name, Checksum sum, byte[] bytes) {
        sum.update(bytes, 0, bytes.length);
        long whole = sum.getValue();
        sum.reset();
        for (byte b : bytes) {
            sum.update(b);
        }
        long single = sum.getValue();
        sum.reset();
        ByteBuffer direct = ByteBuffer.allocateDirect(bytes.length + 8);
        direct.put(new byte[3]).put(bytes).flip().position(3);
        sum.update(direct);
        long buffered = sum.getValue();
        System.out.println(name + " " + Long.toHexString(whole) + " " + (whole == single) + " " + (whole == buffered)
            + " " + direct.position());
    }

    public static void main(String[] args) {
        byte[] text = "Wikipedia, and then a longer text for the checksums to run over".getBytes(StandardCharsets.UTF_8);
        byte[] high = new byte[7000];
        java.util.Arrays.fill(high, (byte) 0xff);
        show("adler32", new Adler32(), text);
        show("crc32", new CRC32(), text);
        show("adler32", new Adler32(), high);
        show("crc32", new CRC32(), high);
    }
}
