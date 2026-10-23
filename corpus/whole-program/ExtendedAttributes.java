import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileSystemException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.DosFileAttributes;
import java.nio.file.attribute.UserDefinedFileAttributeView;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

// User-defined extended attributes through UserDefinedFileAttributeView, their copy by
// Files.copy with COPY_ATTRIBUTES, and the DOS attributes kept in one of them, in a directory of its own under java.io.tmpdir that it removes
// at the end. A file system without user attributes fails every call alike in Java and in the
// translation, which then prints its failures.
public class ExtendedAttributes {
    static String dirPath;

    static String shown(Object o) {
        return String.valueOf(o).replace(dirPath, "<dir>");
    }

    static String read(UserDefinedFileAttributeView view, String name) throws IOException {
        ByteBuffer buf = ByteBuffer.allocate(view.size(name));
        view.read(name, buf);
        buf.flip();
        return StandardCharsets.UTF_8.decode(buf).toString();
    }

    static List<String> sorted(List<String> names) {
        List<String> copy = new ArrayList<>(names);
        Collections.sort(copy);
        return copy;
    }

    public static void main(String[] args) throws IOException {
        Path dir = Path.of(System.getProperty("java.io.tmpdir"), "java-to-rust-xattr-" + System.nanoTime());
        dirPath = dir.toString();
        Files.createDirectory(dir);
        Path file = Files.writeString(dir.resolve("notes.txt"), "some text\n");
        UserDefinedFileAttributeView view = Files.getFileAttributeView(file, UserDefinedFileAttributeView.class);
        System.out.println("view " + view.name());
        try {
            System.out.println("written " + view.write("color", StandardCharsets.UTF_8.encode("blue")));
            view.write("size", ByteBuffer.wrap("a longer value, of more bytes".getBytes(StandardCharsets.UTF_8)));
            System.out.println("names " + sorted(view.list()));
            System.out.println("color " + view.size("color") + " " + read(view, "color"));
            System.out.println("size " + read(view, "size"));
            view.write("color", StandardCharsets.UTF_8.encode("green"));
            System.out.println("rewritten " + read(view, "color"));
            Path copy = dir.resolve("copy.txt");
            Files.copy(file, copy, StandardCopyOption.COPY_ATTRIBUTES);
            UserDefinedFileAttributeView copied = Files.getFileAttributeView(copy, UserDefinedFileAttributeView.class);
            System.out.println("copied " + sorted(copied.list()) + " " + read(copied, "color"));
            view.delete("size");
            System.out.println("after delete " + sorted(view.list()));
            Path dos = Files.writeString(dir.resolve("dos.txt"), "");
            System.out.println("hidden " + Files.getAttribute(dos, "dos:hidden"));
            Files.setAttribute(dos, "dos:hidden", true);
            Files.setAttribute(dos, "dos:archive", true);
            DosFileAttributes attrs = Files.readAttributes(dos, DosFileAttributes.class);
            System.out.println("dos " + attrs.isHidden() + " " + attrs.isArchive() + " " + attrs.isReadOnly()
                    + " " + attrs.isSystem() + " " + attrs.size());
        } catch (FileSystemException e) {
            System.out.println("no user attributes here: " + shown(e));
        }
        try {
            view.size("missing");
        } catch (FileSystemException e) {
            System.out.println("missing: " + shown(e.getFile()) + " " + e.getReason());
        }
        try {
            Files.copy(dir.resolve("absent.txt"), dir.resolve("never.txt"), StandardCopyOption.COPY_ATTRIBUTES);
        } catch (IOException e) {
            System.out.println("copy of a missing file: " + shown(e));
        }
        try (var entries = Files.list(dir)) {
            for (Path p : entries.toList()) {
                Files.delete(p);
            }
        }
        Files.delete(dir);
        System.out.println("removed " + !Files.exists(dir));
    }
}
