import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.OverlappingFileLockException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Comparator;
import java.util.stream.Stream;

/**
 * File locks as a server takes one on its world's directory (Minecraft's `DirectoryLock`): a
 * channel that writes a mark, forces it to the device and locks the file; a second lock of the
 * region, through the same channel or another one of the same file, which the program's table of
 * locks refuses; shared locks of parts of the file; and the locks that closing the channel ends.
 * The channel of a `RandomAccessFile` locks its file too.
 */
public class FileLocks {
    public static void main(String[] args) throws IOException {
        Path dir = Files.createTempDirectory("file-locks");
        try {
            run(dir);
            randomAccess(dir);
        } finally {
            try (Stream<Path> files = Files.walk(dir)) {
                for (Path p : files.sorted(Comparator.reverseOrder()).toList()) {
                    Files.delete(p);
                }
            }
        }
    }

    static void run(Path dir) throws IOException {
        Path lockFile = dir.resolve("session.lock");
        FileChannel channel = FileChannel.open(lockFile, StandardOpenOption.CREATE, StandardOpenOption.WRITE);
        channel.write(ByteBuffer.wrap("☃".getBytes(StandardCharsets.UTF_8)));
        channel.force(true);
        FileLock lock = channel.tryLock();
        System.out.println("locked: " + (lock != null) + ", shared: " + lock.isShared() + ", valid: "
                + lock.isValid() + ", region: " + lock.position() + " + " + lock.size());
        System.out.println("size: " + channel.size());
        try {
            channel.tryLock();
            System.out.println("locked twice");
        } catch (OverlappingFileLockException e) {
            System.out.println("again through the channel: " + e.getClass().getSimpleName());
        }
        try (FileChannel other = FileChannel.open(lockFile, StandardOpenOption.READ, StandardOpenOption.WRITE)) {
            try {
                other.tryLock();
                System.out.println("locked through another channel");
            } catch (OverlappingFileLockException e) {
                System.out.println("again through another channel: " + e.getClass().getSimpleName());
            }
            lock.release();
            System.out.println("released, valid: " + lock.isValid());
            FileLock part = other.lock(0, 2, true);
            FileLock rest = other.tryLock(2, Long.MAX_VALUE - 2, true);
            System.out.println("parts: " + part.isShared() + " " + rest.isShared() + " " + rest.size() + " "
                    + part.overlaps(1, 1) + " " + rest.overlaps(0, 2));
            other.force(false);
            channel.position(0);
            System.out.println("closing the other channel ends its locks: " + part.isValid());
            other.close();
            System.out.println(part.isValid() + " " + rest.isValid());
        }
        FileLock again = channel.lock();
        System.out.println("locked again: " + again.isValid() + " " + again.channel().equals(channel));
        channel.close();
        System.out.println("after closing, valid: " + again.isValid());
        System.out.println("contents: " + Files.readString(lockFile));
    }

    static void randomAccess(Path dir) throws IOException {
        try (RandomAccessFile file = new RandomAccessFile(dir.resolve("data").toFile(), "rw")) {
            FileChannel channel = file.getChannel();
            channel.write(ByteBuffer.wrap(new byte[] {1, 2, 3}));
            channel.force(true);
            channel.force(false);
            FileLock lock = channel.tryLock(0, 3, false);
            System.out.println("random access lock: " + lock.isValid() + " " + lock.size());
            try {
                channel.lock(2, 5, true);
                System.out.println("locked twice");
            } catch (OverlappingFileLockException e) {
                System.out.println("overlapping: " + e.getClass().getSimpleName());
            }
            FileLock after = channel.lock(3, 5, false);
            lock.release();
            System.out.println("released: " + lock.isValid() + " " + after.isValid());
            lock = channel.tryLock(0, 3, true);
            System.out.println("shared: " + lock.isShared());
        }
        System.out.println("length: " + Files.size(dir.resolve("data")));
    }
}
