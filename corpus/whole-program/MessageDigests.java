import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.security.DigestInputStream;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

// Message digests of the JDK's own provider, as javac's `-XDshowClass` computes a class file's
// SHA-256 checksum: `MessageDigest.getInstance` by standard names, aliases and object
// identifiers, in any case; digests of several blocks fed in pieces, through a
// `DigestInputStream` and directly; `reset`, `clone`, `isEqual` and an unknown name.
public class MessageDigests {
    static String hex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }

    static byte[] data(int length) {
        byte[] bytes = new byte[length];
        for (int i = 0; i < length; i++) {
            bytes[i] = (byte) (i * 31 + (i >> 8));
        }
        return bytes;
    }

    // As javac's `Main.showClass` reads a class file.
    static byte[] throughStream(InputStream in, String algorithm) throws IOException, NoSuchAlgorithmException {
        MessageDigest md = MessageDigest.getInstance(algorithm);
        try (DigestInputStream din = new DigestInputStream(in, md)) {
            byte[] buf = new byte[8192];
            int n;
            do {
                n = din.read(buf);
            } while (n > 0);
        }
        return md.digest();
    }

    public static void main(String[] args) throws Exception {
        String[] names = {
            "SHA-256", "sha-256", "SHA256", "SHA-224", "SHA-1", "SHA", "sha1", "MD5", "MD2", "SHA-384",
            "SHA-512", "SHA-512/224", "SHA512/256", "SHA3-224", "SHA3-256", "SHA3-384", "SHA3-512",
            "2.16.840.1.101.3.4.2.1", "OID.1.3.14.3.2.26", "1.2.840.113549.2.5",
        };
        byte[] abc = "abc".getBytes(StandardCharsets.US_ASCII);
        byte[] big = data(100_000);
        for (String name : names) {
            MessageDigest md = MessageDigest.getInstance(name);
            System.out.println(md.getAlgorithm() + " " + md.getDigestLength());
            System.out.println("  empty " + hex(md.digest()));
            System.out.println("  abc   " + hex(md.digest(abc)));
            for (int i = 0; i < big.length; i += 777) {
                md.update(big, i, Math.min(777, big.length - i));
            }
            System.out.println("  big   " + hex(md.digest()));
        }

        MessageDigest md = MessageDigest.getInstance("SHA-256");
        md.update((byte) 'a');
        MessageDigest copy = (MessageDigest) md.clone();
        md.update("bc".getBytes(StandardCharsets.US_ASCII));
        copy.update((byte) 'b');
        copy.update((byte) 'c');
        byte[] first = md.digest();
        System.out.println("clone " + MessageDigest.isEqual(first, copy.digest()) + " " + hex(first));
        md.update(big);
        md.reset();
        System.out.println("reset " + hex(md.digest(abc)));
        byte[] out = new byte[40];
        md.update(abc);
        System.out.println("into " + md.digest(out, 4, 32) + " " + hex(out));

        System.out.println("stream " + hex(throughStream(new ByteArrayInputStream(big), "SHA-256")));
        MessageDigest off = MessageDigest.getInstance("SHA-256");
        DigestInputStream din = new DigestInputStream(new ByteArrayInputStream(abc), off);
        din.on(false);
        System.out.println("off " + din.read() + " " + din.getMessageDigest().getAlgorithm());
        din.on(true);
        System.out.println("on " + din.read() + " " + din.read() + " " + din.read());
        System.out.println("bc " + hex(off.digest()));
        System.out.println(din);

        try {
            MessageDigest.getInstance("SHA-2");
        } catch (NoSuchAlgorithmException e) {
            System.out.println(e);
        }
    }
}
