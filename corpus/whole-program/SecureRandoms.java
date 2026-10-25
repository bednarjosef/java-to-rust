import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.util.HashSet;
import java.util.Set;
import java.util.UUID;

// Secure random numbers of the JDK's own provider: the default `new SecureRandom()`, which on
// Linux mixes `/dev/urandom` with a SHA-1 generator, the generators named by `getInstance` in any
// case, a seeded SHA-1 generator, whose output is fixed by its seed, and `UUID.randomUUID`. Only
// what does not depend on the random bytes is printed: algorithms, names, ranges, lengths, a
// version and a variant, and whether values differ.
public class SecureRandoms {
    static String hex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }

    public static void main(String[] args) throws Exception {
        SecureRandom random = new SecureRandom();
        System.out.println(random.getAlgorithm() + " " + random);
        int n = random.nextInt(10);
        System.out.println("nextInt(10) in range " + (n >= 0 && n < 10));
        byte[] bytes = new byte[100];
        random.nextBytes(bytes);
        Set<Byte> distinct = new HashSet<>();
        for (byte b : bytes) {
            distinct.add(b);
        }
        System.out.println("distinct bytes > 30 " + (distinct.size() > 30));
        System.out.println("seed length " + random.generateSeed(16).length);
        double d = random.nextDouble();
        System.out.println("nextDouble in range " + (d >= 0 && d < 1));

        for (String name : new String[] {"NativePRNG", "nativeprng", "NativePRNGNonBlocking", "SHA1PRNG"}) {
            SecureRandom named = SecureRandom.getInstance(name);
            System.out.println(name + " -> " + named.getAlgorithm() + " " + named);
            System.out.println("  long differs " + (named.nextLong() != named.nextLong()));
        }
        try {
            SecureRandom.getInstance("NoSuchRandom");
        } catch (NoSuchAlgorithmException e) {
            System.out.println(e.getMessage());
        }

        // SHA1PRNG seeded before its first use is deterministic.
        SecureRandom seeded = SecureRandom.getInstance("SHA1PRNG");
        seeded.setSeed(new byte[] {1, 2, 3, 4});
        byte[] out = new byte[32];
        seeded.nextBytes(out);
        System.out.println("SHA1PRNG seeded " + hex(out));
        System.out.println("SHA1PRNG next int " + seeded.nextInt());

        Set<UUID> uuids = new HashSet<>();
        for (int i = 0; i < 1000; i++) {
            UUID uuid = UUID.randomUUID();
            if (uuid.version() != 4 || uuid.variant() != 2) {
                System.out.println("bad " + uuid.version() + " " + uuid.variant());
            }
            uuids.add(uuid);
        }
        System.out.println("distinct uuids " + uuids.size());
    }
}
