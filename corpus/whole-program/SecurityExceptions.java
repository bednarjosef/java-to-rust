import java.security.GeneralSecurityException;
import java.security.InvalidKeyException;
import java.security.InvalidParameterException;
import java.security.KeyException;
import java.security.KeyStoreException;
import java.security.SignatureException;
import java.security.cert.CertificateException;
import java.security.cert.CertificateParsingException;
import java.security.spec.InvalidKeySpecException;

// The JDK's security exceptions thrown and caught by the program's own code, as a reader of key
// files does: their messages, causes and superclasses are Java's.
public class SecurityExceptions {
    static String readKey(String text) throws KeyException {
        if (text.isEmpty()) {
            throw new KeyException("could not find a PKCS #8 private key");
        }
        if (text.startsWith("-")) {
            throw new InvalidKeyException("bad key", new IllegalStateException("dash"));
        }
        return text.toUpperCase();
    }

    static void check(int step) throws GeneralSecurityException {
        switch (step) {
            case 0 -> throw new CertificateException("no certificate");
            case 1 -> throw new CertificateParsingException(new IllegalArgumentException("parse"));
            case 2 -> throw new SignatureException();
            case 3 -> throw new KeyStoreException("store", new RuntimeException("inner"));
            case 4 -> throw new InvalidKeySpecException("spec");
            default -> System.out.println("checked " + step);
        }
    }

    public static void main(String[] args) {
        for (String text : new String[] {"key", "", "-x"}) {
            try {
                System.out.println(readKey(text));
            } catch (KeyException e) {
                System.out.println(e + " cause " + e.getCause());
            }
        }
        for (int step = 0; step < 6; step++) {
            try {
                check(step);
            } catch (CertificateException e) {
                System.out.println("certificate: " + e.getMessage());
            } catch (GeneralSecurityException e) {
                System.out.println("security: " + e + " / " + e.getCause());
            }
        }
        try {
            throw new InvalidParameterException("parameter");
        } catch (IllegalArgumentException e) {
            System.out.println(e);
        }
    }
}
