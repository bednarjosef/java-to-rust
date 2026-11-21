// Lambdas that capture nothing, evaluated in a hot loop: each is one object per place it is
// written, as Java's runtime makes it, so every evaluation hands out that object again. A class
// implements the interface too, so the lambdas are objects of it rather than plain closures.
public class ConstantLambdas {
    interface Rule { boolean test(int x); }

    static final class Below implements Rule {
        final int limit;
        Below(int limit) { this.limit = limit; }
        public boolean test(int x) { return x < limit; }
    }

    static final Rule BELOW = new Below(100_000);

    static Rule pick(int i) {
        switch (i & 3) {
            case 0: return x -> x > 1000;
            case 1: return x -> (x & 1) == 0;
            case 2: return x -> x % 3 == 0;
            default: return (i & 4) == 0 ? BELOW : x -> x > 7;
        }
    }

    public static void main(String[] args) {
        long start = System.nanoTime();
        long hits = 0;
        Rule last = null;
        int changes = 0;
        for (int i = 0; i < 50_000_000; i++) {
            Rule rule = pick(i ^ (i >>> 7));
            if (rule.test(i & 0x3ffff)) hits++;
            if (rule != last) changes++;
            last = rule;
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        System.out.println(hits + " " + changes);
    }
}
