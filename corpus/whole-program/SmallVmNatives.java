// Inflater.inflate(ByteBuffer) on a zlib stream written in the source, through direct input and
// output buffers, whose memory the runtime reads and writes by address. The text read back must
// be the original, and the counts and flags of the inflater must match.
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.zip.DataFormatException;
import java.util.zip.Inflater;

public class SmallVmNatives {
    // The zlib stream of "Inflated through direct buffers, by the runtime, in native memory.",
    // as java.util.zip.Deflater writes it.
    static final byte[] STREAM = {
        (byte) 0x78, (byte) 0x9c, (byte) 0x0d, (byte) 0xc9, (byte) 0xd1, (byte) 0x0d, (byte) 0x80, (byte) 0x20,
        (byte) 0x0c, (byte) 0x05, (byte) 0xc0, (byte) 0x55, (byte) 0xde, (byte) 0x00, (byte) 0xc4, (byte) 0x3d,
        (byte) 0x1c, (byte) 0x03, (byte) 0xe4, (byte) 0x55, (byte) 0x9a, (byte) 0x48, (byte) 0x49, (byte) 0x4a,
        (byte) 0x31, (byte) 0x61, (byte) 0x7b, (byte) 0xbd, (byte) 0xdf, (byte) 0x3b, (byte) 0x4d, (byte) 0x9e,
        (byte) 0x1c, (byte) 0xac, (byte) 0x88, (byte) 0xe6, (byte) 0x63, (byte) 0xdd, (byte) 0x0d, (byte) 0x55,
        (byte) 0x9d, (byte) 0x57, (byte) 0xa0, (byte) 0x2c, (byte) 0x11, (byte) 0xfa, (byte) 0x4c, (byte) 0x28,
        (byte) 0xfb, (byte) 0x2f, (byte) 0xc2, (byte) 0x97, (byte) 0x85, (byte) 0x76, (byte) 0x26, (byte) 0xa8,
        (byte) 0xc1, (byte) 0x72, (byte) 0xe8, (byte) 0x4b, (byte) 0x74, (byte) 0xf6, (byte) 0xe1, (byte) 0xfb,
        (byte) 0xf8, (byte) 0x00, (byte) 0x35, (byte) 0xc4, (byte) 0x18, (byte) 0x4e,
    };

    public static void main(String[] args) throws DataFormatException {
        ByteBuffer input = ByteBuffer.allocateDirect(STREAM.length);
        input.put(STREAM).flip();
        Inflater inflater = new Inflater();
        inflater.setInput(input);
        ByteBuffer output = ByteBuffer.allocateDirect(128);
        int n = inflater.inflate(output);
        System.out.println(n + " " + inflater.finished() + " " + inflater.getRemaining() + " " + inflater.getBytesWritten());
        output.flip();
        byte[] text = new byte[output.remaining()];
        output.get(text);
        System.out.println(new String(text, StandardCharsets.UTF_8));
        inflater.end();
    }
}
