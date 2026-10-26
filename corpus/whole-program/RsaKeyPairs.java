import java.math.BigInteger;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.NoSuchAlgorithmException;
import java.security.interfaces.RSAPrivateCrtKey;
import java.security.interfaces.RSAPublicKey;

// RSA key pairs from KeyPairGenerator, as a server makes one as it starts: what the keys say of
// themselves, and that the private key undoes the public one.
public class RsaKeyPairs {
    public static void main(String[] args) throws Exception {
        KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
        System.out.println(generator.getAlgorithm());
        generator.initialize(1024);
        KeyPair pair = generator.generateKeyPair();
        RSAPublicKey pub = (RSAPublicKey) pair.getPublic();
        RSAPrivateCrtKey priv = (RSAPrivateCrtKey) pair.getPrivate();
        System.out.println(pub.getAlgorithm() + " " + pub.getFormat() + " " + pub.getEncoded().length);
        System.out.println(priv.getAlgorithm() + " " + priv.getFormat());
        System.out.println(pub.getModulus().bitLength() + " " + pub.getPublicExponent());
        System.out.println(priv.getPrimeP().multiply(priv.getPrimeQ()).equals(pub.getModulus()));
        BigInteger message = new BigInteger("123456789012345678901234567890");
        BigInteger sealed = message.modPow(pub.getPublicExponent(), pub.getModulus());
        System.out.println(sealed.modPow(priv.getPrivateExponent(), priv.getModulus()).equals(message));
        System.out.println(pub.equals(pair.getPublic()) + " " + pub.equals(priv));

        KeyPairGenerator other = KeyPairGenerator.getInstance("rsa");
        other.initialize(512);
        System.out.println(((RSAPublicKey) other.generateKeyPair().getPublic()).getModulus().bitLength());
        try {
            KeyPairGenerator.getInstance("NoSuchKeys");
        } catch (NoSuchAlgorithmException e) {
            System.out.println(e.getMessage());
        }
        try {
            generator.initialize(100);
        } catch (Exception e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
