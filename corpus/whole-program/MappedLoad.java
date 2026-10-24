// Loading a mapping of a file, which reads a byte of each of its pages from the page's start.
import java.io.File;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.MappedByteBuffer;
import java.nio.channels.FileChannel;

public class MappedLoad {
    public static void main(String[] args) throws IOException {
        File file = new File(System.getProperty("java.io.tmpdir"), "java-to-rust-mapped-" + System.nanoTime());
        try (RandomAccessFile raf = new RandomAccessFile(file, "rw")) {
            byte[] bytes = new byte[10000];
            for (int i = 0; i < bytes.length; i++) {
                bytes[i] = (byte) (i * 7);
            }
            raf.write(bytes);
            MappedByteBuffer map = raf.getChannel().map(FileChannel.MapMode.READ_ONLY, 5000, 5000);
            map.load();
            System.out.println(map.isLoaded() + " " + map.get(0) + " " + map.get(4999));
            System.out.println(map.get(1234));
        } finally {
            file.delete();
        }
    }
}
