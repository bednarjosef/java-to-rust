// The security APIs' exceptions and reflection's `UndeclaredThrowableException`, made, thrown and
// caught as library code does: their messages, causes and superclasses.
import java.lang.reflect.UndeclaredThrowableException;
import java.security.GeneralSecurityException;
import java.security.InvalidAlgorithmParameterException;
import java.security.InvalidKeyException;
import java.security.InvalidParameterException;
import java.security.KeyException;
import java.security.KeyManagementException;
import java.security.KeyStoreException;
import java.security.NoSuchProviderException;
import java.security.SignatureException;
import java.security.UnrecoverableKeyException;
import java.security.cert.CertificateException;
import java.security.cert.CertificateParsingException;
import java.security.spec.InvalidKeySpecException;

public class SecurityExceptions {
    static void fail(Exception e) throws Exception {
        throw e;
    }

    public static void main(String[] args) throws Exception {
        Exception[] all = {
            new KeyException("bad key"), new KeyStoreException("store"), new NoSuchProviderException("prov"),
            new SignatureException("sig", new RuntimeException("why")), new InvalidKeyException(),
            new InvalidAlgorithmParameterException("alg"), new CertificateException("cert"),
            new CertificateParsingException("parse"), new InvalidKeySpecException("spec"),
            new UnrecoverableKeyException("key"), new KeyManagementException("mgmt"),
        };
        for (Exception e : all) {
            try {
                fail(e);
            } catch (KeyException k) {
                System.out.println("key: " + k + " " + k.getCause());
            } catch (GeneralSecurityException s) {
                System.out.println("security: " + s + " " + s.getCause());
            }
        }
        try {
            throw new InvalidParameterException("param");
        } catch (IllegalArgumentException e) {
            System.out.println(e);
        }
        UndeclaredThrowableException u = new UndeclaredThrowableException(new java.io.IOException("io"));
        System.out.println(u + " " + u.getCause() + " " + u.getUndeclaredThrowable() + " " + u.getMessage());
        try {
            u.initCause(null);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
        try {
            throw new UndeclaredThrowableException(null, "none");
        } catch (UndeclaredThrowableException e) {
            System.out.println(e + " " + e.getUndeclaredThrowable());
        }
    }
}
