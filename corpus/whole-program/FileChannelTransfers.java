import java.io.IOException;
import java.nio.MappedByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

// Files copied and read through file channels, in a temporary directory that it removes at the end:
// Files.copy of a file larger than a transfer's buffer, transferTo and transferFrom between two
// channels at positions (the positions of each channel after the transfer are printed), and a
// read-only mapping of a region that starts inside a page, whose bytes are printed.
public class FileChannelTransfers {
    public static void main(String[] args) throws IOException {
        Path dir = Files.createTempDirectory("nio-channels");
        Path source = dir.resolve("source.txt");
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 20000; i++) {
            text.append((char) ('a' + i % 26));
        }
        Files.writeString(source, text.toString());

        Path copy = dir.resolve("copy.txt");
        Files.copy(source, copy);
        System.out.println(Files.size(copy) + " " + Files.readString(copy).equals(text.toString()));

        Path target = dir.resolve("target.txt");
        try (FileChannel in = FileChannel.open(source, StandardOpenOption.READ);
                FileChannel out = FileChannel.open(target, StandardOpenOption.CREATE, StandardOpenOption.WRITE)) {
            long moved = in.transferTo(5000, 3000, out);
            System.out.println(moved + " " + in.position() + " " + out.position());
            long rest = in.transferTo(19000, 10000, out);
            System.out.println(rest + " " + in.position() + " " + out.position() + " " + out.size());
        }
        System.out.println(Files.readString(target).equals(text.substring(5000, 8000) + text.substring(19000)));

        Path back = dir.resolve("back.txt");
        try (FileChannel from = FileChannel.open(source, StandardOpenOption.READ);
                FileChannel to = FileChannel.open(back, StandardOpenOption.CREATE, StandardOpenOption.WRITE,
                        StandardOpenOption.READ)) {
            long got = to.transferFrom(from, 10, 4000);
            System.out.println(got + " " + from.position() + " " + to.position() + " " + to.size());
            System.out.println(to.read(java.nio.ByteBuffer.allocate(3), 10) + " "
                    + Files.readString(back).substring(10, 13));
        }

        try (FileChannel in = FileChannel.open(copy, StandardOpenOption.READ)) {
            MappedByteBuffer map = in.map(FileChannel.MapMode.READ_ONLY, 4090, 20);
            byte[] bytes = new byte[map.remaining()];
            map.get(bytes);
            System.out.println(new String(bytes, StandardCharsets.US_ASCII));
            System.out.println(map.capacity() + " " + map.isReadOnly() + " " + in.position());
        }

        Files.delete(copy);
        Files.delete(target);
        Files.delete(back);
        Files.delete(source);
        Files.delete(dir);
        System.out.println(Files.exists(dir));
    }
}
