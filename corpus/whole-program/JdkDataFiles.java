import java.net.IDN;

// Data files of the JDK that are made when the JDK is built and are not in its sources, which the
// translator reads from the JDK's image: the IDN profile `uidna.spp` (`IDN`, which every HTTPS
// connection's host name goes through) and the Unicode names of `uniName.dat`
// (`Character.getName`, `Character.codePointOf`).
public class JdkDataFiles {
    public static void main(String[] args) {
        System.out.println(IDN.toASCII("bücher.example"));
        System.out.println(IDN.toUnicode("xn--bcher-kva.example").equals("bücher.example"));
        System.out.println(IDN.toASCII("Example.COM"));
        System.out.println(Character.getName('A'));
        System.out.println(Character.getName(0x1F600));
        System.out.println(Character.codePointOf("latin small letter e with acute"));
        try {
            IDN.toASCII("a\u0000b", IDN.USE_STD3_ASCII_RULES);
        } catch (IllegalArgumentException e) {
            System.out.println("rejected");
        }
    }
}
