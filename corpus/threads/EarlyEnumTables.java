// Enums whose constants are built from other enums' constants and constants of other classes
// (their fields become tables that the enum's static initialization fills), read from worker
// threads in a loop. Those whose initialization nothing can observe may run before main does
// anything, and are then read with no test of whether they are initialized; an enum whose
// constructor prints, and those whose constants or constructors name that enum's constants, must
// still initialize at their first use, in Java's order.
public class EarlyEnumTables {
    static final class Bits {
        static final int TYPE = 1;
        static final int VALUE = 2;
        static final int METHOD = 4;
    }

    enum Category {
        BASIC,
        ERROR,
        TARGET
    }

    // Built from constants of another enum and of another class.
    enum Selector {
        TYP(Category.BASIC, Bits.TYPE),
        VAL(Category.BASIC, Bits.VALUE),
        MTH(Category.TARGET, Bits.METHOD),
        ERR(Category.ERROR, Bits.TYPE | Bits.VALUE | Bits.METHOD);

        final Category category;
        final int bits;

        Selector(Category category, int bits) {
            this.category = category;
            this.bits = bits;
        }

        boolean contains(Selector other) {
            return (bits & other.bits) == other.bits;
        }
    }

    // Built from constants of Selector, an enum of the same kind, and its own constants.
    enum Kind {
        NIL(Selector.ERR, null),
        TYPE(Selector.TYP, NIL),
        VAR(Selector.VAL, NIL),
        METHOD(Selector.MTH, VAR),
        CTOR(Selector.MTH, METHOD);

        static final int SPAN = CTOR.ordinal() - TYPE.ordinal() + 1;

        final Selector selector;
        final Kind outer;
        final int weight;

        Kind(Selector selector, Kind outer) {
            this(selector, outer, 3);
        }

        Kind(Selector selector, Kind outer, int weight) {
            this.selector = selector;
            this.outer = outer;
            this.weight = weight;
        }

        int depth() {
            int d = 0;
            for (Kind k = this; k.outer != null; k = k.outer) {
                d += k.weight;
            }
            return d;
        }
    }

    // Its short constructors pass the long one a constant of Category, as javac's TokenKind passes
    // Tag.DEFAULT.
    enum Token {
        EOF,
        IDENT(Category.TARGET),
        ARROW("->"),
        DOT(".", Category.ERROR);

        final String name;
        final Category category;

        Token() {
            this(null, Category.BASIC);
        }

        Token(String name) {
            this(name, Category.BASIC);
        }

        Token(Category category) {
            this(null, category);
        }

        Token(String name, Category category) {
            this.name = name;
            this.category = category;
        }
    }

    // Prints as it builds its constants: it initializes at its first use.
    enum Noisy {
        LOUD("loud"),
        QUIET("quiet");

        final String word;

        Noisy(String word) {
            this.word = word;
            System.out.println("Noisy." + word + " built");
        }
    }

    // Built from Noisy's constants: initializing it initializes Noisy, which prints.
    enum Echo {
        FIRST(Noisy.LOUD, 1),
        SECOND(Noisy.QUIET, 2);

        final Noisy source;
        final int times;

        Echo(Noisy source, int times) {
            this.source = source;
            this.times = times;
        }
    }

    // Its constructor passes a constant of Noisy to the other one: initializing it initializes
    // Noisy, which prints.
    enum Relay {
        NEAR,
        FAR;

        final Noisy via;

        Relay() {
            this(Noisy.QUIET);
        }

        Relay(Noisy via) {
            this.via = via;
        }
    }

    static long walk(int seed, int rounds) {
        Kind[] kinds = Kind.values();
        Token[] tokens = Token.values();
        long sum = 0;
        for (int i = 0; i < rounds; i++) {
            Kind k = kinds[(seed + i) % kinds.length];
            Token token = tokens[(seed + i) % tokens.length];
            if (token.category == Category.BASIC) {
                sum += 5;
            }
            if (token.name != null) {
                sum += token.name.length();
            }
            sum += k.depth() + k.selector.bits + Kind.SPAN;
            if (k.selector.contains(Selector.TYP)) {
                sum += 7;
            }
            if (k.selector.category == Category.TARGET && k != Kind.CTOR) {
                sum += 11;
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println("start");
        long[] sums = new long[3];
        Thread[] threads = new Thread[3];
        for (int t = 0; t < threads.length; t++) {
            int k = t;
            threads[t] = new Thread(() -> sums[k] = walk(k, 100000 + k));
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        System.out.println("sums " + sums[0] + " " + sums[1] + " " + sums[2]);
        System.out.println("before relay");
        System.out.println("relay " + Relay.FAR.via.word);
        System.out.println("before echo");
        System.out.println("echo " + Echo.SECOND.source.word + " x" + Echo.SECOND.times);
        System.out.println("noisy " + Noisy.LOUD.word);
        System.out.println("kind " + Kind.CTOR + " " + Kind.CTOR.depth() + " " + Kind.CTOR.selector.category);
        System.out.println("token " + Token.DOT + " " + Token.DOT.name + " " + Token.IDENT.category);
    }
}
