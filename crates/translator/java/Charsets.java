// The charsets of the JDK that builds the translator, which `build.rs` asks for and writes as
// `charsets.json` in the build directory: each charset's canonical name and aliases, and for a
// charset that maps each byte to one char and back (a single-byte charset, such as windows-1252),
// what it decodes each byte to and encodes each char to. A translated program carries these
// tables, so that `Charset.forName`, `getBytes` and `new String` of such a charset behave as on
// this JDK (`lower/charsets.rs`, `runtime/src/charset.rs`). Nothing here is written by hand: the
// tables are this JDK's answers.
//
// The translator runs it again when it runs with another JDK than the one it was built with.
//
// A charset is a table only when this JDK's coders behave as one: every byte decodes to one
// char, alone or among others; every char encodes to one byte or is unmappable, alone or among
// others; a surrogate pair or a lone surrogate becomes one replacement byte; and no char but the
// one that the replacement byte decodes to encodes to that byte. The table holds
// what each byte decodes to (U+FFFD for a byte without a char) and the chars whose encoding is
// not the inverse of that, the first byte of each char winning: a char that encodes to another
// byte, and one that does not encode at all.

import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.ByteBuffer;
import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CharsetEncoder;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Random;
import java.util.TreeSet;

public class Charsets {
    public static void main(String[] args) {
        StringBuilder out = new StringBuilder();
        // The JDK, as `java -XshowSettings:properties` names it, which the translator compares
        // with the JDK it runs with (`jdk_charsets.rs`).
        String jdk = System.getProperty("java.home") + " " + System.getProperty("java.runtime.version");
        out.append("{\"jdk\":").append(quote(jdk)).append(",\"charsets\":[\n");
        boolean first = true;
        for (Charset cs : Charset.availableCharsets().values()) {
            if (!first) {
                out.append(",\n");
            }
            first = false;
            out.append("{\"name\":").append(quote(cs.name()));
            // The historical name, which `getEncoding` of a reader gives.
            String historical = new InputStreamReader(InputStream.nullInputStream(), cs).getEncoding();
            out.append(",\"historical\":").append(quote(historical)).append(",\"aliases\":[");
            boolean firstAlias = true;
            for (String alias : new TreeSet<>(cs.aliases())) {
                out.append(firstAlias ? "" : ",").append(quote(alias));
                firstAlias = false;
            }
            out.append("]");
            Table table = table(cs);
            if (table != null) {
                out.append(",\"decode\":").append(Arrays.toString(table.decode).replace(" ", ""));
                out.append(",\"overrides\":[");
                for (int i = 0; i < table.overrides.size(); i++) {
                    int[] o = table.overrides.get(i);
                    out.append(i == 0 ? "" : ",").append('[').append(o[0]).append(',').append(o[1]).append(']');
                }
                out.append("],\"replacement\":").append(table.replacement & 0xff);
            }
            out.append("}");
        }
        out.append("\n]}\n");
        System.out.print(out);
    }

    static final class Table {
        final int[] decode = new int[256];
        final List<int[]> overrides = new ArrayList<>();
        byte replacement;
    }

    /** The table of `cs`, or null where its coders do not behave as one. */
    static Table table(Charset cs) {
        if (!cs.canEncode()) {
            return null;
        }
        CharsetEncoder encoder = cs.newEncoder();
        CharsetDecoder decoder = cs.newDecoder();
        if (encoder.maxBytesPerChar() != 1.0f || decoder.maxCharsPerByte() != 1.0f) {
            return null;
        }
        byte[] replacement = encoder.replacement();
        if (replacement.length != 1) {
            return null;
        }
        Table table = new Table();
        table.replacement = replacement[0];
        byte[] all = new byte[256];
        for (int b = 0; b < 256; b++) {
            all[b] = (byte) b;
            String one = new String(new byte[] {(byte) b}, cs);
            if (one.length() != 1) {
                return null;
            }
            table.decode[b] = one.charAt(0);
        }
        // Each byte decodes the same among others.
        Random random = new Random(1);
        byte[] mixed = new byte[4096];
        random.nextBytes(mixed);
        if (!decodeModel(table, all).equals(new String(all, cs))
            || !decodeModel(table, mixed).equals(new String(mixed, cs))) {
            return null;
        }
        // The encoding of each char, alone; the first byte of a char is the inverse's.
        int[] inverse = new int[65536];
        Arrays.fill(inverse, -1);
        for (int b = 0; b < 256; b++) {
            int c = table.decode[b];
            if (c != 0xFFFD && inverse[c] < 0) {
                inverse[c] = b;
            }
        }
        int[] encode = new int[65536];
        for (int c = 0; c < 65536; c++) {
            if (Character.isSurrogate((char) c)) {
                encode[c] = -1;
                continue;
            }
            byte[] bytes = String.valueOf((char) c).getBytes(cs);
            if (bytes.length != 1) {
                return null;
            }
            boolean mapped = cs.newEncoder().canEncode((char) c);
            if (!mapped && bytes[0] != table.replacement) {
                return null;
            }
            encode[c] = mapped ? bytes[0] & 0xff : -1;
            if (encode[c] != inverse[c]) {
                table.overrides.add(new int[] {c, encode[c]});
            }
            // Only the char of the replacement byte encodes to it, so that a whole program's
            // coders (`TranslatedCoders`) can tell an unmappable char by the replacement.
            if (encode[c] == (table.replacement & 0xff) && c != table.decode[table.replacement & 0xff]) {
                return null;
            }
        }
        // Each char encodes the same among others, surrogates included.
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 8192; i++) {
            int pick = random.nextInt(10);
            if (pick == 0) {
                text.appendCodePoint(0x10000 + random.nextInt(0x100000));
            } else if (pick == 1) {
                text.append((char) (0xD800 + random.nextInt(0x800)));
            } else if (pick < 6) {
                text.append((char) table.decode[random.nextInt(256)]);
            } else {
                text.append((char) random.nextInt(0x10000));
            }
        }
        for (int c = 0; c < 65536; c++) {
            text.append((char) c);
        }
        String s = text.toString();
        if (!Arrays.equals(encodeModel(table, encode, s), s.getBytes(cs))) {
            return null;
        }
        return table;
    }

    static String decodeModel(Table table, byte[] bytes) {
        StringBuilder s = new StringBuilder();
        for (byte b : bytes) {
            s.append((char) table.decode[b & 0xff]);
        }
        return s.toString();
    }

    /** What the runtime does with the table: a pair or a lone surrogate is one replacement. */
    static byte[] encodeModel(Table table, int[] encode, String s) {
        ByteBuffer out = ByteBuffer.allocate(s.length());
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (Character.isHighSurrogate(c) && i + 1 < s.length() && Character.isLowSurrogate(s.charAt(i + 1))) {
                i++;
                out.put(table.replacement);
            } else if (encode[c] < 0) {
                out.put(table.replacement);
            } else {
                out.put((byte) encode[c]);
            }
        }
        return Arrays.copyOf(out.array(), out.position());
    }

    static String quote(String s) {
        StringBuilder q = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') {
                q.append('\\');
            }
            q.append(c);
        }
        return q.append('"').toString();
    }
}
