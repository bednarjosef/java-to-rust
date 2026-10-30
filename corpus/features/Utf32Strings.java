import java.nio.charset.Charset;
import java.util.Arrays;

// The UTF-32 charsets, which DER's universal strings use: byte order marks when decoding (each
// charset takes its own, `UTF-32` either), code points above U+10FFFF and bytes left at the end
// replaced, a surrogate kept as it is.
public class Utf32Strings {
    static String units(String s) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            out.append(' ').append(Integer.toHexString(s.charAt(i)));
        }
        return out.toString();
    }

    public static void main(String[] args) {
        String[] names = {"UTF-32", "UTF-32BE", "UTF-32LE", "utf32", "X-UTF-32LE"};
        for (String name : names) {
            Charset cs = Charset.forName(name);
            byte[] bytes = "Aé😀".getBytes(cs);
            System.out.println(cs.name() + " " + Arrays.toString(bytes) + units(new String(bytes, cs)));
        }
        byte[] big = {0, 0, (byte) 0xfe, (byte) 0xff, 0, 0, 0, 0x41};
        byte[] little = {(byte) 0xff, (byte) 0xfe, 0, 0, 0x41, 0, 0, 0};
        for (String name : new String[] {"UTF-32", "UTF-32BE", "UTF-32LE"}) {
            Charset cs = Charset.forName(name);
            System.out.println(name + ":" + units(new String(big, cs)) + " /" + units(new String(little, cs)));
        }
        Charset utf32 = Charset.forName(args.length > 0 ? args[0] : "UTF-32");
        byte[] odd = {0, 0x11, 0, 0, 0, 0, (byte) 0xd8, 0x3d, 0, 0, 0, 0x42, 7};
        System.out.println(units(new String(odd, utf32)) + " " + units(new String(odd, 4, 8, utf32)));
        System.out.println(Arrays.toString("x\ud800y".getBytes(utf32)));
        System.out.println(Charset.forName("UTF_32BE").equals(Charset.forName("X-UTF-32BE")) + units(new String(big, 4, 4, Charset.forName("UTF-32BE"))));
    }
}
