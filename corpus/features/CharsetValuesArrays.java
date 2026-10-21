import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

public class CharsetValuesArrays {
    static Charset[] table = {StandardCharsets.UTF_8, StandardCharsets.UTF_16, StandardCharsets.US_ASCII};

    static Charset first(Charset[] all) {
        return all[0];
    }

    public static void main(String[] args) {
        Charset[] all = {StandardCharsets.UTF_16BE, StandardCharsets.UTF_16LE, StandardCharsets.ISO_8859_1};
        for (Charset cs : all) {
            System.out.println(cs.name() + " " + cs.hashCode());
        }
        for (Charset cs : table) {
            System.out.println(cs.displayName());
        }
        System.out.println(first(all) == StandardCharsets.UTF_16BE);
        System.out.println(first(table).equals(StandardCharsets.UTF_8));
        all[1] = table[2];
        System.out.println(all[1].name() + " " + all.length);
        Charset[] more = new Charset[]{StandardCharsets.UTF_8, all[0]};
        System.out.println(more[1] + " " + Arrays.toString(more.length == 2 ? new int[]{more.length} : new int[0]));
    }
}
