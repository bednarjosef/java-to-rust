import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.Dictionary;
import java.util.Hashtable;

// Charsets kept in a raw table and cast back where they are read, as the JDK's `RTFReader` reads
// its font charsets (`(Charset) this.fcharsetTable.get(Integer.valueOf(n))`, from an inner class).
public class CharsetsFromATable {
    Dictionary<Object, Object> table = new Hashtable<>();

    class Reader {
        Charset charsetOf(int n) {
            Charset charset = (Charset) table.get(Integer.valueOf(n));
            return charset != null ? charset : StandardCharsets.US_ASCII;
        }
    }

    public static void main(String[] args) {
        CharsetsFromATable outer = new CharsetsFromATable();
        outer.table.put(Integer.valueOf(1), StandardCharsets.UTF_8);
        outer.table.put(Integer.valueOf(2), Charset.forName("ISO-8859-1"));
        outer.table.put(Integer.valueOf(3), "not a charset");
        Reader reader = outer.new Reader();
        for (int n = 0; n < 4; n++) {
            try {
                System.out.println(n + " " + reader.charsetOf(n));
            } catch (ClassCastException e) {
                System.out.println(n + " not a charset");
            }
        }
        outer.table = null;
        try {
            reader.charsetOf(1);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
