// Static fields whose names differ only in case, as the Benchmarks Game's fasta writes
// `static final int SEED = 42; static int seed = SEED;`: Rust's statics are upper case, so the
// later one gets a number after its name.
public class StaticsNamedAlikeButCase {
    static final int SEED = 42;
    static int seed = SEED;
    static String Seed = "s";
    static int seed_2 = 7;
    static final String NAME = "name";
    static String name = NAME + "!";

    static int next(int max) {
        seed = (seed * 3877 + 29573) % 139968;
        return max * seed / 139968;
    }

    public static void main(String[] args) {
        System.out.println(next(100) + " " + next(100) + " " + SEED + " " + seed);
        Seed += seed_2;
        seed_2++;
        System.out.println(Seed + " " + seed_2 + " " + NAME + " " + name);
        switch (args.length + 42) {
            case SEED -> System.out.println("the seed");
            default -> System.out.println("other");
        }
    }
}
