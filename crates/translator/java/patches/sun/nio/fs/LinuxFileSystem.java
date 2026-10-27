package sun.nio.fs;

import java.io.FileInputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/**
 * The translator's patch of the JDK's {@code LinuxFileSystem}: the mount table (see
 * docs/jdk-overlays.md).
 *
 * <p>The JDK reads {@code /etc/mtab} or {@code /proc/mounts} through the C library's
 * {@code setmntent} and {@code getmntent_r}, on a {@code FILE*} that only the JVM's natives can
 * hold. Here the table is read whole and each line is split as {@code getmntent_r} splits it
 * ({@link UnixMountEntry#parse}). A table that cannot be read has no entries, as in the JDK.
 */
class LinuxFileSystem {
    @Replace
    List<UnixMountEntry> getMountEntries(String fstab) {
        ArrayList<UnixMountEntry> entries = new ArrayList<>();
        byte[] table;
        try (FileInputStream in = new FileInputStream(fstab)) {
            table = in.readAllBytes();
        } catch (IOException x) {
            return entries;
        }
        int start = 0;
        while (start < table.length) {
            int end = start;
            while (end < table.length && table[end] != '\n') {
                end++;
            }
            UnixMountEntry entry = UnixMountEntry.parse(table, start, end);
            if (entry != null) {
                entries.add(entry);
            }
            start = end + 1;
        }
        return entries;
    }
}
