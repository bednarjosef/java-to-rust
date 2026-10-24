// A file system's space through `File` (0 for a path that does not exist), and a weak reference,
// whose class starts the reference handler thread that waits for ever for references to process.
import java.io.File;
import java.lang.ref.WeakReference;

public class FileSpaceAndReferences {
    public static void main(String[] args) {
        File dir = new File(".");
        long total = dir.getTotalSpace(), free = dir.getFreeSpace(), usable = dir.getUsableSpace();
        System.out.println((total > 0) + " " + (free <= total) + " " + (usable <= free));
        System.out.println(new File("/no/such/place").getTotalSpace());
        Object held = new Object();
        WeakReference<Object> weak = new WeakReference<>(held);
        System.out.println(weak.get() == held);
    }
}
