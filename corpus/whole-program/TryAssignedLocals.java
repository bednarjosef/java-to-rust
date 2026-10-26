import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PushbackInputStream;
import java.io.StringWriter;
import java.io.Writer;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.function.Supplier;

// Locals declared without a value and set in a `try`, as the Minecraft server sets them: inside an
// outer `try`, in a `try` whose `catch` returns, in both branches of an `if` with one of them in
// a try-with-resources (`SavedDataStorage.readTagFromDisk`), in both branches of an `if` inside
// a `try` (`PalettedContainer.unpack`), and in a try-with-resources or a `try` inside one.
public class TryAssignedLocals {
    static class BadData extends Exception {
        BadData(String m) { super(m); }
    }

    static final class Dynamic<T> {
        final T value;
        Dynamic(T value) { this.value = value; }
        Dynamic<T> fix() { return new Dynamic<>(value); }
        public String toString() { return "Dynamic(" + value + ")"; }
    }

    static final class Tag {
        final String text;
        Tag(String text) { this.text = text; }
        public String toString() { return "Tag(" + text + ")"; }
    }

    interface BitStorage { int size(); }
    record ZeroBitStorage(int size) implements BitStorage {}
    record SimpleBitStorage(int size, long[] data) implements BitStorage {}
    interface Palette<T> { String name(); }
    record Named<T>(String name) implements Palette<T> {}

    static Dynamic<?> unfixed(boolean fail) throws IOException {
        if (fail) throw new IOException("corrupt");
        return new Dynamic<>("data");
    }

    static String load(boolean has, boolean fail) {
        try {
            Dynamic<?> levelDataTag;
            if (has) {
                Dynamic<?> levelDataUnfixed;
                try {
                    levelDataUnfixed = unfixed(fail);
                } catch (IOException | IllegalStateException ex) {
                    System.out.println("failed: " + ex.getMessage());
                    return "none";
                }
                System.out.println("summary " + levelDataUnfixed);
                levelDataTag = levelDataUnfixed.fix();
            } else {
                levelDataTag = null;
            }
            Supplier<String> later = () -> levelDataTag != null ? levelDataTag.toString() : "new";
            return later.get();
        } catch (Exception e) {
            return "crashed";
        }
    }

    static Tag readTag(byte[] bytes) throws IOException {
        try (
            InputStream in = new ByteArrayInputStream(bytes);
            PushbackInputStream inputStream = new PushbackInputStream(in, 2)
        ) {
            Tag tag;
            int first = inputStream.read();
            if (first == 31) {
                tag = new Tag("gzip");
            } else {
                inputStream.unread(first);
                try (DataInputStream dis = new DataInputStream(inputStream)) {
                    tag = new Tag("plain " + dis.readByte());
                }
            }
            return new Tag(tag.text + "!");
        }
    }

    static <T> String unpack(int bits, Optional<long[]> dataOpt, boolean repack) {
        BitStorage storage;
        Palette<T> palette;
        if (bits == 0) {
            palette = new Named<>("single");
            storage = new ZeroBitStorage(4);
        } else {
            if (dataOpt.isEmpty()) {
                return "missing";
            }
            long[] data = dataOpt.get();
            try {
                if (!repack) {
                    palette = new Named<>("linear");
                    storage = check(data);
                } else {
                    Palette<T> old = new Named<>("hash");
                    BitStorage oldStorage = check(data);
                    palette = new Named<>(old.name() + oldStorage.size());
                    storage = new SimpleBitStorage(oldStorage.size() * 2, data);
                }
            } catch (BadData exception) {
                return "failed: " + exception.getMessage();
            }
        }
        return palette.name() + " " + storage.size();
    }

    static BitStorage check(long[] data) throws BadData {
        if (data.length > 3) throw new BadData("too long");
        return new SimpleBitStorage(data.length, data);
    }

    static String dump(boolean fail) throws IOException {
        StringWriter text = new StringWriter();
        try (Writer output = text) {
            List<String> modules;
            try {
                modules = new ArrayList<>(listModules(fail));
            } catch (Throwable t) {
                System.out.println("Failed to list native modules");
                return "";
            }
            modules.sort(String.CASE_INSENSITIVE_ORDER);
            for (String module : modules) {
                output.write(module);
                output.write(10);
            }
        }
        return text.toString();
    }

    static List<String> listModules(boolean fail) {
        if (fail) throw new IllegalStateException("no modules");
        return List.of("b", "A", "c");
    }

    static String certificate(byte[] bytes) {
        String certificate;
        try (InputStream inputStream = new ByteArrayInputStream(bytes)) {
            certificate = "cert" + inputStream.read();
        } catch (Exception e) {
            System.out.println("Failed to open certificate file");
            return null;
        }
        return certificate.toUpperCase();
    }

    public static void main(String[] args) throws IOException {
        System.out.println(load(true, false));
        System.out.println(load(true, true));
        System.out.println(load(false, false));
        System.out.println(readTag(new byte[] {31, 2}));
        System.out.println(readTag(new byte[] {5, 2}));
        System.out.println(unpack(0, Optional.empty(), false));
        System.out.println(unpack(4, Optional.empty(), false));
        System.out.println(unpack(4, Optional.of(new long[] {1, 2}), false));
        System.out.println(unpack(4, Optional.of(new long[] {1, 2}), true));
        System.out.println(unpack(4, Optional.of(new long[] {1, 2, 3, 4}), true));
        System.out.print(dump(false));
        System.out.println(dump(true).isEmpty());
        System.out.println(certificate(new byte[] {7}));
        System.out.println(certificate(new byte[0]));
    }
}
