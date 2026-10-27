import java.io.IOException;
import java.nio.file.FileStore;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * The file stores of the mount table, as a server looks up the one its world is on: the store of
 * a path, its name, type and options, and the stores the default file system lists. The machine
 * decides the values, which Java and the translation read from the same table.
 */
public class MountedFileStores {
    public static void main(String[] args) throws IOException {
        for (String p : new String[] {"/", "/proc", "/tmp"}) {
            FileStore store = Files.getFileStore(Path.of(p));
            System.out.println(p + ": " + store.name() + " " + store.type() + " read-only " + store.isReadOnly()
                    + " posix " + store.supportsFileAttributeView("posix"));
        }
        // The whole table changes as things are mounted, which a stored run of Java would not
        // see, so only what holds of it is printed.
        FileStore root = Files.getFileStore(Path.of("/"));
        boolean listed = false;
        boolean typed = true;
        for (FileStore store : FileSystems.getDefault().getFileStores()) {
            listed |= store.equals(root);
            typed &= !store.type().isEmpty();
        }
        System.out.println("the root's store is listed: " + listed + ", every store has a type: " + typed);
    }
}
