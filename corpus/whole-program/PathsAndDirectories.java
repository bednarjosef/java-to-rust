import java.io.IOException;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

// java.nio.file's paths and directories, in a directory of its own under java.io.tmpdir that it
// removes at the end: making directories, testing what exists, relative paths, listing a
// directory stream, and deleting, with the failure of a missing file.
public class PathsAndDirectories {
    public static void main(String[] args) throws IOException {
        Path dir = Path.of(System.getProperty("java.io.tmpdir"), "nio-" + System.nanoTime());
        Files.createDirectory(dir);
        Path sub = dir.resolve("sub");
        Files.createDirectories(sub.resolve("deeper"));
        Path file = dir.resolve("a.txt");
        System.out.println(Files.exists(file) + " " + Files.isDirectory(dir) + " " + Files.isDirectory(sub));
        System.out.println(dir.relativize(sub.resolve("deeper")) + " " + Paths.get("x", "y").getFileName());
        List<String> names = new ArrayList<>();
        try (DirectoryStream<Path> entries = Files.newDirectoryStream(dir)) {
            for (Path p : entries) {
                names.add(p.getFileName().toString());
            }
        }
        Collections.sort(names);
        System.out.println(names);
        try {
            Files.delete(dir.resolve("none"));
        } catch (NoSuchFileException e) {
            System.out.println("missing: " + dir.relativize(Path.of(e.getMessage())));
        }
        System.out.println(Files.deleteIfExists(dir.resolve("none")));
        Files.delete(sub.resolve("deeper"));
        Files.delete(sub);
        Files.delete(dir);
        System.out.println(Files.exists(dir));
    }
}
