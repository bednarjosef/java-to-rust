import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.FileReader;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.util.Arrays;

// Files and directories through java.io, in a directory of its own under java.io.tmpdir that it
// removes at the end: making, listing, reading, writing at positions, attributes, renaming and
// deleting, and the failures of a missing file.
public class FilesAndDirectories {
    static String dirPath;

    static String shown(String text) {
        return text.replace(dirPath, "<dir>");
    }

    public static void main(String[] args) throws IOException {
        File dir = new File(System.getProperty("java.io.tmpdir"), "java-to-rust-fs-" + System.nanoTime());
        dirPath = dir.getPath();
        System.out.println("mkdir " + dir.mkdir() + " again " + dir.mkdir());
        File nested = new File(dir, "a/b/c");
        System.out.println("mkdirs " + nested.mkdirs() + " " + nested.isDirectory());

        File text = new File(dir, "notes.txt");
        System.out.println("created " + text.createNewFile() + " again " + text.createNewFile());
        try (FileOutputStream out = new FileOutputStream(text)) {
            out.write("first line\nsecond line\n".getBytes());
        }
        try (FileOutputStream out = new FileOutputStream(text, true)) {
            out.write("third\n".getBytes());
        }
        System.out.println("length " + text.length() + " file " + text.isFile() + " hidden " + text.isHidden());

        try (FileInputStream in = new FileInputStream(text)) {
            byte[] buf = new byte[6];
            int n = in.read(buf);
            StringBuilder first = new StringBuilder();
            for (int i = 0; i < n; i++) {
                first.append((char) buf[i]);
            }
            System.out.println("read " + n + " " + first + " available " + in.available());
            System.out.println("skip " + in.skip(5) + " next " + (char) in.read());
        }
        try (BufferedReader reader = new BufferedReader(new FileReader(text))) {
            String line;
            int count = 0;
            while ((line = reader.readLine()) != null) {
                count++;
                System.out.println(count + ": " + line);
            }
        }

        File data = new File(dir, "data.bin");
        try (RandomAccessFile raf = new RandomAccessFile(data, "rw")) {
            raf.writeInt(0x01020304);
            raf.writeLong(-2L);
            raf.writeUTF("héllo");
            System.out.println("pointer " + raf.getFilePointer() + " length " + raf.length());
            raf.seek(4);
            System.out.println("long " + raf.readLong());
            raf.seek(0);
            System.out.println("int " + Integer.toHexString(raf.readInt()));
            raf.seek(12);
            System.out.println("utf " + raf.readUTF());
            raf.setLength(6);
            System.out.println("cut " + raf.length() + " pointer " + raf.getFilePointer() + " read " + raf.read());
            raf.seek(20);
            raf.write(7);
            System.out.println("grown " + raf.length());
        }
        try (RandomAccessFile raf = new RandomAccessFile(data, "r")) {
            raf.write(1);
        } catch (IOException e) {
            System.out.println("read-only write: " + e.getMessage());
        }

        String[] names = dir.list();
        Arrays.sort(names);
        System.out.println(Arrays.toString(names));
        File[] files = dir.listFiles();
        Arrays.sort(files);
        for (File f : files) {
            System.out.println(f.getName() + " dir " + f.isDirectory() + " file " + f.isFile() + " exists " + f.exists());
        }

        System.out.println("modified set " + text.setLastModified(1_000_000_000_000L) + " " + text.lastModified());
        System.out.println("readable " + text.canRead() + " writable " + text.canWrite() + " executable " + text.canExecute());
        System.out.println("executable set " + text.setExecutable(true) + " " + text.canExecute());
        System.out.println("read-only set " + text.setReadOnly() + " writable " + text.canWrite());
        System.out.println("writable set " + text.setWritable(true) + " " + text.canWrite());

        File moved = new File(dir, "moved.txt");
        System.out.println("rename " + text.renameTo(moved) + " old " + text.exists() + " new " + moved.exists());
        System.out.println("canonical " + shown(new File(dir, "a/./b/../../moved.txt").getCanonicalPath().replace(dir.getCanonicalPath(), dirPath)));
        System.out.println("parent " + shown(moved.getParent()) + " name " + moved.getName() + " absolute " + moved.isAbsolute());

        File missing = new File(dir, "missing.txt");
        System.out.println("missing " + missing.exists() + " " + missing.length() + " " + missing.lastModified() + " " + missing.delete() + " " + (missing.list() == null));
        try (FileInputStream in = new FileInputStream(missing)) {
            System.out.println("opened " + in.read());
        } catch (FileNotFoundException e) {
            System.out.println("not found: " + shown(e.getMessage()));
        }
        try (RandomAccessFile raf = new RandomAccessFile(dir, "r")) {
            System.out.println("opened " + raf.length());
        } catch (FileNotFoundException e) {
            System.out.println("directory: " + shown(e.getMessage()));
        }
        try {
            new File(dir, "none/x.txt").createNewFile();
        } catch (IOException e) {
            System.out.println("create: " + e.getMessage());
        }

        System.out.println("delete non-empty " + dir.delete());
        System.out.println("delete " + moved.delete() + " " + data.delete() + " " + nested.delete()
                + " " + new File(dir, "a/b").delete() + " " + new File(dir, "a").delete() + " " + dir.delete());
        System.out.println("gone " + !dir.exists());
    }
}
