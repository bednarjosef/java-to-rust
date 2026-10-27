package sun.nio.fs;

import java.io.ByteArrayOutputStream;

/**
 * The translator's patch of the JDK's {@code UnixMountEntry}: an entry made from a line of the
 * mount table, which the JDK's native fills from {@code getmntent_r} (see
 * {@code LinuxFileSystem}'s patch).
 */
class UnixMountEntry {
    /**
     * The entry of the line {@code table[from..to)}, split as glibc's {@code getmntent_r} splits
     * it, or null for a line that is blank or a comment: whitespace before and after is dropped,
     * the fields (name, directory, type, options) are separated by runs of spaces and tabs, a
     * missing one is empty, and the escapes {@code \040}, {@code \011}, {@code \012},
     * {@code \134} and {@code \\} in them are the bytes they name.
     */
    @Add
    static UnixMountEntry parse(byte[] table, int from, int to) {
        while (from < to && isBlank(table[from])) {
            from++;
        }
        while (to > from && isBlank(table[to - 1])) {
            to--;
        }
        if (from == to || table[from] == '#') {
            return null;
        }
        byte[][] fields = new byte[4][];
        int at = from;
        for (int i = 0; i < fields.length; i++) {
            int end = at;
            while (end < to && !isBlank(table[end])) {
                end++;
            }
            fields[i] = decode(table, at, end);
            at = end;
            while (at < to && isBlank(table[at])) {
                at++;
            }
        }
        UnixMountEntry entry = new UnixMountEntry();
        entry.name = fields[0];
        entry.dir = fields[1];
        entry.fstype = fields[2];
        entry.opts = fields[3];
        return entry;
    }

    @Add
    private static boolean isBlank(byte b) {
        return b == ' ' || b == '\t';
    }

    /** The bytes of a field, with the escapes of {@code getmntent_r} decoded. */
    @Add
    private static byte[] decode(byte[] table, int from, int to) {
        ByteArrayOutputStream out = new ByteArrayOutputStream(to - from);
        for (int i = from; i < to; i++) {
            byte b = table[i];
            if (b == '\\' && i + 1 < to && table[i + 1] == '\\') {
                out.write('\\');
                i++;
                continue;
            }
            if (b == '\\' && i + 3 < to) {
                int escaped = switch (new String(table, i + 1, 3, java.nio.charset.StandardCharsets.ISO_8859_1)) {
                    case "040" -> ' ';
                    case "011" -> '\t';
                    case "012" -> '\n';
                    case "134" -> '\\';
                    default -> -1;
                };
                if (escaped >= 0) {
                    out.write(escaped);
                    i += 3;
                    continue;
                }
            }
            out.write(b);
        }
        return out.toByteArray();
    }
}
