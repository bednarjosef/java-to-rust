import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.NoSuchProviderException;
import java.security.Provider;
import java.security.SecureRandom;
import java.security.Security;
import java.util.HexFormat;
import javax.crypto.Mac;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;
import javax.net.ssl.KeyManagerFactory;
import javax.net.ssl.SSLContext;
import javax.net.ssl.TrustManagerFactory;

// Security services looked up by name, as the JDK's providers register them: the providers
// themselves, services of several types by name, alias and provider name, and the exceptions
// for names that no provider knows.
public class SecurityServicesByName {
    interface Lookup {
        Object run() throws Exception;
    }

    static void attempt(String what, Lookup lookup) {
        try {
            System.out.println(what + ": " + lookup.run());
        } catch (Exception e) {
            System.out.println(what + ": " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    static String hex(byte[] bytes) {
        return HexFormat.of().formatHex(bytes);
    }

    public static void main(String[] args) throws Exception {
        Provider sun = Security.getProvider("SUN");
        System.out.println(sun.getName() + " " + (sun.getVersionStr() != null));
        System.out.println("SunJCE: " + Security.getProvider("SunJCE").getName());
        System.out.println("BC: " + Security.getProvider("BC"));
        System.out.println("SUN has SHA-256: " + (sun.getService("MessageDigest", "SHA-256") != null));
        System.out.println("SUN has sha256 alias: " + (sun.getService("MessageDigest", "SHA256") != null));
        System.out.println("SUN has AES: " + (sun.getService("Cipher", "AES") != null));
        System.out.println("keystore.type: " + Security.getProperty("keystore.type"));
        System.out.println("default key store: " + KeyStore.getDefaultType());
        System.out.println("key managers: " + KeyManagerFactory.getDefaultAlgorithm());
        System.out.println("trust managers: " + TrustManagerFactory.getDefaultAlgorithm());

        KeyStore keys = KeyStore.getInstance("JCEKS");
        keys.load(null, null);
        System.out.println(keys.getType() + " " + keys.getProvider().getName() + " " + keys.size());
        attempt("unknown key store", () -> KeyStore.getInstance("NoSuchStore"));

        SecureRandom random = SecureRandom.getInstance("SHA1PRNG", "SUN");
        System.out.println(random.getAlgorithm() + " " + random.getProvider().getName());
        attempt("random of another provider", () -> SecureRandom.getInstance("SHA1PRNG", "SunJCE"));
        attempt("random of no provider", () -> SecureRandom.getInstance("SHA1PRNG", "NoSuchProvider"));

        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec("key".getBytes(), "HmacSHA256"));
        System.out.println(mac.getAlgorithm() + " " + mac.getProvider().getName() + " "
                + hex(mac.doFinal("message".getBytes())));
        Mac byOid = Mac.getInstance("1.2.840.113549.2.9");
        System.out.println("by object identifier: " + byOid.getAlgorithm());
        attempt("unknown mac", () -> Mac.getInstance("HmacNothing"));

        SecretKeyFactory factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256");
        byte[] derived = factory.generateSecret(
                new PBEKeySpec("password".toCharArray(), "salt".getBytes(), 1000, 128)).getEncoded();
        System.out.println(factory.getAlgorithm() + " " + hex(derived));

        attempt("unknown ssl context", () -> SSLContext.getInstance("NoSuchProtocol"));
        attempt("unknown digest", () -> MessageDigest.getInstance("NoSuchDigest"));
    }
}
