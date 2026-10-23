import java.io.FileInputStream;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.channels.ClosedByInterruptException;
import java.nio.channels.ClosedChannelException;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

// The channels of RandomAccessFile and FileInputStream (the JDK's FileChannelImpl): reads, writes,
// positions and sizes, and a read by a thread that is interrupted already, which closes the channel
// and throws ClosedByInterruptException. The file is in a directory of its own under
// java.io.tmpdir, removed at the end.
public class FileChannelsOfStreams {
    static String text(ByteBuffer buf) {
        buf.flip();
        return StandardCharsets.UTF_8.decode(buf).toString();
    }

    public static void main(String[] args) throws IOException {
        Path dir = Files.createDirectory(Path.of(System.getProperty("java.io.tmpdir"),
                "java-to-rust-channels-" + System.nanoTime()));
        Path file = Files.writeString(dir.resolve("data.txt"), "0123456789abcdefghij");
        try (RandomAccessFile raf = new RandomAccessFile(file.toFile(), "rw")) {
            FileChannel ch = raf.getChannel();
            System.out.println("size " + ch.size() + " position " + ch.position());
            ByteBuffer buf = ByteBuffer.allocate(8);
            System.out.println("read " + ch.read(buf) + " " + text(buf) + " position " + ch.position());
            ch.position(15);
            buf.clear();
            System.out.println("read " + ch.read(buf) + " " + text(buf));
            buf.clear();
            System.out.println("at the end " + ch.read(buf));
            System.out.println("written " + ch.write(ByteBuffer.wrap("XYZ".getBytes(StandardCharsets.UTF_8)), 2));
            System.out.println("file pointer " + raf.getFilePointer() + " size " + ch.size());
            ch.truncate(12);
            ByteBuffer direct = ByteBuffer.allocateDirect(32);
            System.out.println("read at 0 " + ch.read(direct, 0) + " " + text(direct));
        }
        try (FileInputStream in = new FileInputStream(file.toFile())) {
            FileChannel ch = in.getChannel();
            in.read();
            in.read();
            System.out.println("stream position " + ch.position());
            ByteBuffer buf = ByteBuffer.allocate(4);
            ch.read(buf);
            System.out.println("after the stream's reads " + text(buf) + " " + in.read());
            try {
                ch.write(ByteBuffer.allocate(1));
            } catch (Exception e) {
                System.out.println("write to a reading channel: " + e);
            }
        }
        RandomAccessFile raf = new RandomAccessFile(file.toFile(), "r");
        FileChannel ch = raf.getChannel();
        Thread.currentThread().interrupt();
        try {
            ch.read(ByteBuffer.allocate(4));
        } catch (ClosedByInterruptException e) {
            System.out.println("interrupted: " + e + " open " + ch.isOpen() + " still interrupted "
                    + Thread.interrupted());
        }
        try {
            ch.read(ByteBuffer.allocate(4));
        } catch (ClosedChannelException e) {
            System.out.println("closed: " + e);
        }
        raf.close();
        Files.delete(file);
        Files.delete(dir);
        System.out.println("removed " + !Files.exists(dir));
    }
}
