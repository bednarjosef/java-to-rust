// Calls on a variable of a class that the program never creates an object of, which only ever
// holds null: an inherited method that takes its object's handle (JNA's `PSID.ByReference`
// calling `PSID.getBytes`), an abstract method returning an array that no class implements
// (`X509Certificate.getEncoded` in msal4j's `ClientCertificate`), each throwing a
// `NullPointerException`, and the text of one in a concatenation, whose `toString` it inherits
// from a class of other objects (a `ServerPlayer` field in Minecraft's `TestCommand`): `null`.
import java.security.cert.CertificateEncodingException;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.List;

public class CallsOnClassesWithoutObjects {
    static final List<Sid> SEEN = new ArrayList<>();

    static class Sid {
        final byte[] bytes;

        Sid(byte[] bytes) {
            this.bytes = bytes;
        }

        byte[] getBytes() {
            SEEN.add(this);
            return bytes.clone();
        }

        static class ByReference extends Sid {
            ByReference() {
                super(new byte[] {9});
            }
        }
    }

    abstract static class Entity {
        final String name;

        Entity(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return getClass().getSimpleName() + "[" + name + "]";
        }
    }

    static class Pig extends Entity {
        Pig() {
            super("pig");
        }
    }

    static class Cow extends Entity {
        Cow() {
            super("cow");
        }
    }

    static class Player extends Entity {
        Player() {
            super("player");
        }
    }

    static final class Group {
        Player player;
        Entity seen;

        Sid.ByReference primary;
        X509Certificate certificate;
        final List<X509Certificate> chain = new ArrayList<>();

        int encodedLength() throws CertificateEncodingException {
            return certificate.getEncoded().length;
        }

        List<Integer> chainLengths() throws CertificateEncodingException {
            List<Integer> out = new ArrayList<>();
            for (X509Certificate c : chain) {
                out.add(c.getEncoded().length);
            }
            return out;
        }
    }

    public static void main(String[] args) throws Exception {
        Sid sid = new Sid(new byte[] {1, 2});
        System.out.println(sid.getBytes().length + " " + SEEN.size());
        Group group = new Group();
        try {
            System.out.println(group.primary.getBytes().length);
        } catch (NullPointerException e) {
            System.out.println("no primary " + SEEN.size());
        }
        System.out.println(group.chainLengths());
        try {
            System.out.println(group.encodedLength());
        } catch (NullPointerException e) {
            System.out.println("no certificate");
        }
        group.seen = args.length > 5 ? new Cow() : new Pig();
        System.out.println("player=" + group.player + ", seen=" + group.seen + ", " + new Cow());
    }
}
