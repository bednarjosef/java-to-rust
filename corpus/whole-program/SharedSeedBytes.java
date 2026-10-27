import java.security.SecureRandom;

// `SecureRandom.getSeed` in a program whose `byte[]`s two threads reach, as the JDK's
// `ThreadLocalRandom` seeds from it in a server that runs many: the seed is made a shared array.
public class SharedSeedBytes {
    static byte[] filled;

    public static void main(String[] args) throws Exception {
        Thread filler = new Thread(() -> {
            filled = new byte[] {1, 2, 3};
        });
        filler.start();
        filler.join();
        byte[] seed = SecureRandom.getSeed(8);
        System.out.println(seed.length + " " + filled.length + " " + filled[2]);
        filled = seed;
        System.out.println(filled.length);
    }
}
