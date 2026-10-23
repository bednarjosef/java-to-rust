import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

// Symbolic links: made, read, and copied as links (Files.copy with NOFOLLOW_LINKS, which reads
// the link and makes a new one) or through them, in a directory of its own under java.io.tmpdir,
// removed at the end.
public class SymbolicLinkCopies {
    public static void main(String[] args) throws IOException {
        Path dir = Files.createDirectory(Path.of(System.getProperty("java.io.tmpdir"),
                "java-to-rust-links-" + System.nanoTime()));
        Path file = Files.writeString(dir.resolve("target.txt"), "linked text\n");
        Path link = Files.createSymbolicLink(dir.resolve("link"), Path.of("target.txt"));
        System.out.println("link " + Files.isSymbolicLink(link) + " to " + Files.readSymbolicLink(link));
        Path copy = Files.copy(link, dir.resolve("copy"), LinkOption.NOFOLLOW_LINKS);
        System.out.println("copy is a link " + Files.isSymbolicLink(copy) + " to " + Files.readSymbolicLink(copy)
                + ": " + Files.readString(copy).strip());
        Path through = Files.copy(link, dir.resolve("through"));
        System.out.println("copy through it is a link " + Files.isSymbolicLink(through) + ": "
                + Files.readString(through).strip());
        Path moved = Files.move(copy, dir.resolve("moved"), StandardCopyOption.ATOMIC_MOVE);
        System.out.println("moved " + Files.isSymbolicLink(moved) + " " + Files.exists(copy, LinkOption.NOFOLLOW_LINKS));
        try {
            Files.createSymbolicLink(link, file);
        } catch (IOException e) {
            System.out.println("again: " + e.getClass().getSimpleName());
        }
        for (Path p : new Path[] {link, moved, through, file}) {
            Files.delete(p);
        }
        Files.delete(dir);
        System.out.println("removed " + !Files.exists(dir));
    }
}
