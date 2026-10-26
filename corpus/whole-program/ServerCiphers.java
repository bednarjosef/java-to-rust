import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.PublicKey;
import java.security.Signature;
import java.security.spec.X509EncodedKeySpec;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

// The ciphers, key factories and signatures of a Minecraft server's login: the client's shared
// key sealed with the server's RSA key, the connection encrypted with AES/CFB8 under it, a public
// key read back from its X.509 encoding, and chat signed with SHA256withRSA.
public class ServerCiphers {
    public static void main(String[] args) throws Exception {
        KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
        generator.initialize(1024);
        KeyPair pair = generator.generateKeyPair();

        KeyGenerator keys = KeyGenerator.getInstance("AES");
        keys.init(128);
        SecretKey shared = keys.generateKey();
        System.out.println(shared.getAlgorithm() + " " + shared.getFormat() + " " + shared.getEncoded().length);

        Cipher seal = Cipher.getInstance(pair.getPublic().getAlgorithm());
        seal.init(Cipher.ENCRYPT_MODE, pair.getPublic());
        byte[] sealed = seal.doFinal(shared.getEncoded());
        Cipher open = Cipher.getInstance("RSA");
        open.init(Cipher.DECRYPT_MODE, pair.getPrivate());
        byte[] opened = open.doFinal(sealed);
        System.out.println(sealed.length + " " + Arrays.equals(opened, shared.getEncoded()));

        SecretKey key = new SecretKeySpec(new byte[] {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16}, "AES");
        Cipher out = Cipher.getInstance("AES/CFB8/NoPadding");
        out.init(Cipher.ENCRYPT_MODE, key, new IvParameterSpec(key.getEncoded()));
        Cipher in = Cipher.getInstance("AES/CFB8/NoPadding");
        in.init(Cipher.DECRYPT_MODE, key, new IvParameterSpec(key.getEncoded()));
        byte[] text = "a packet of the game's protocol".getBytes();
        byte[] first = out.update(text, 0, 10);
        byte[] rest = out.update(text, 10, text.length - 10);
        System.out.println(Arrays.toString(first) + " " + rest.length);
        byte[] back = new byte[text.length];
        int n = in.update(first, 0, first.length, back, 0);
        n += in.update(rest, 0, rest.length, back, n);
        System.out.println(n + " " + new String(back));

        PublicKey read = KeyFactory.getInstance("RSA").generatePublic(new X509EncodedKeySpec(pair.getPublic().getEncoded()));
        System.out.println(read.equals(pair.getPublic()) + " " + read.getAlgorithm());

        for (String algorithm : new String[] {"SHA256withRSA", "SHA1withRSA"}) {
            Signature signer = Signature.getInstance(algorithm);
            signer.initSign(pair.getPrivate());
            signer.update("hello".getBytes());
            byte[] signature = signer.sign();
            Signature verifier = Signature.getInstance(algorithm);
            verifier.initVerify(read);
            verifier.update("hello".getBytes());
            boolean good = verifier.verify(signature);
            verifier.initVerify(read);
            verifier.update("hellO".getBytes());
            System.out.println(algorithm + " " + signature.length + " " + good + " " + verifier.verify(signature));
        }
    }
}
