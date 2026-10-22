import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.List;

// A file's contents through java.nio.file, in a temporary directory that it removes at the end:
// bytes and text read and written whole, appended, by lines, through buffered readers and writers
// and through streams, a new empty file, and the failures of an existing, a missing and a
// directory path.
public class FileContentsThroughFiles {
    public static void main(String[] args) throws IOException {
        Path dir = Files.createTempDirectory("nio-contents");
        Path file = dir.resolve("notes.txt");
        Files.writeString(file, "first line\nsecond line\n");
        System.out.println(Files.size(file) + " " + Files.isRegularFile(file));
        System.out.print(Files.readString(file));
        Files.writeString(file, "third line\n", StandardOpenOption.APPEND);
        List<String> lines = Files.readAllLines(file);
        System.out.println(lines.size() + " " + lines.get(2));
        Files.write(file, new byte[] {104, 105, 10});
        byte[] bytes = Files.readAllBytes(file);
        System.out.println(bytes.length + " " + new String(bytes, StandardCharsets.UTF_8).trim());
        try (BufferedWriter out = Files.newBufferedWriter(file)) {
            out.write("written by a writer");
            out.newLine();
        }
        try (BufferedReader in = Files.newBufferedReader(file)) {
            System.out.println(in.readLine() + " / " + in.readLine());
        }
        try (OutputStream out = Files.newOutputStream(dir.resolve("raw.bin"))) {
            for (int i = 0; i < 300; i++) {
                out.write(i);
            }
        }
        try (InputStream in = Files.newInputStream(dir.resolve("raw.bin"))) {
            System.out.println(in.available() + " " + in.read() + " " + in.skip(254) + " " + in.read());
        }
        Path empty = Files.createFile(dir.resolve("empty"));
        System.out.println(Files.size(empty));
        try {
            Files.createFile(empty);
        } catch (FileAlreadyExistsException e) {
            System.out.println("exists: " + empty.getFileName().toString().equals(Path.of(e.getFile()).getFileName().toString()));
        }
        try {
            Files.readAllBytes(dir.resolve("missing"));
        } catch (NoSuchFileException e) {
            System.out.println("missing: " + e.getMessage().endsWith("missing"));
        }
        try {
            Files.readAllBytes(dir);
        } catch (IOException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
        for (String name : List.of("notes.txt", "raw.bin", "empty")) {
            Files.delete(dir.resolve(name));
        }
        Files.delete(dir);
        System.out.println(Files.exists(dir));
    }
}
