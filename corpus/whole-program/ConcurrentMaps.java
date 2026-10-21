// The JDK's ConcurrentHashMap on one thread, whose table reads and updates its bins through
// Unsafe at element offsets of the array (`((long) i << ASHIFT) + ABASE`), and ThreadLocalRandom
// and Random, which keep their seeds through field offsets: ThreadLocalRandom in fields of the
// Thread, a class other than its own. The random numbers differ from run to run, so the program
// prints only what holds for every run, and what a fixed seed makes.
import java.util.Random;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ThreadLocalRandom;

public class ConcurrentMaps {
    public static void main(String[] args) {
        ConcurrentHashMap<String, Integer> counts = new ConcurrentHashMap<>();
        String[] words = {"oak", "elm", "oak", "ash", "elm", "oak", "yew"};
        for (String w : words) {
            counts.merge(w, 1, Integer::sum);
        }
        System.out.println(counts.get("oak") + " " + counts.get("elm") + " " + counts.get("ash") + " " + counts.size());
        System.out.println(counts.putIfAbsent("fir", 9) + " " + counts.putIfAbsent("oak", 9));
        counts.computeIfPresent("ash", (k, v) -> v + 40);
        counts.computeIfAbsent("pine", k -> k.length());
        System.out.println(counts.remove("elm") + " " + counts.containsKey("elm") + " " + counts.getOrDefault("elm", -1));
        System.out.println(counts.replace("yew", 1, 5) + " " + counts.replace("yew", 1, 6) + " " + counts.get("yew"));

        // Enough keys that the table grows several times, moving every bin.
        ConcurrentHashMap<Integer, Integer> squares = new ConcurrentHashMap<>();
        for (int i = 0; i < 500; i++) {
            squares.put(i, i * i);
        }
        long sum = 0;
        for (int i = 0; i < 500; i++) {
            sum += squares.get(i);
        }
        System.out.println(squares.size() + " " + sum + " " + squares.get(321) + " " + squares.get(500));

        ThreadLocalRandom random = ThreadLocalRandom.current();
        int inRange = 0;
        for (int i = 0; i < 1000; i++) {
            int n = random.nextInt(10, 20);
            if (n >= 10 && n < 20) {
                inRange++;
            }
        }
        long bounded = random.nextLong(1000);
        System.out.println(inRange + " " + (bounded >= 0 && bounded < 1000) + " " + (ThreadLocalRandom.current() == random));

        Random seeded = new Random(42);
        System.out.println(seeded.nextInt(100) + " " + seeded.nextLong() + " " + seeded.nextBoolean());
    }
}
