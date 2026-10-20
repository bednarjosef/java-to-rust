@interface Tag {
    String value() default "";
}

@Tag("outer")
@SuppressWarnings("unused")
public class IgnoredSyntax {
    private static final int LIMIT = 3;
    private static volatile int ticks = 0;
    private transient int skipped = 7;
    @Deprecated
    static int old = 1;

    @Tag(value = "method")
    @SuppressWarnings({"unchecked", "rawtypes"})
    static synchronized void tick() {
        ticks++;
    }

    static int calls() {
        ticks++;
        return ticks;
    }

    @Override
    public String toString() {
        return "IgnoredSyntax " + skipped;
    }

    static strictfp double half(final double x) {
        return x / 2;
    }

    static int twice(@Tag final int n) {
        return n * 2;
    }

    public static void main(String[] args) {
        @SuppressWarnings("unused") final int local = 4;
        var count = 0;
        var names = new String[] {"ab", "c"};
        for (final String n : names) {
            count += n.length();
        }
        var total = local + count;
        System.out.println(total);
        String lock = "lock";
        synchronized (lock) {
            System.out.println("locked " + lock.length());
        }
        assert calls() < 0 : "not evaluated";
        System.out.println(ticks);
        System.out.println(calls());
        ;
        for (int i = 0; i < LIMIT; i++)
            ;
        tick();
        System.out.println(ticks + " " + old + " " + half(5.0) + " " + twice(21));
        System.out.println(new IgnoredSyntax());
        String missing = null;
        synchronized (missing) {
            System.out.println("unreachable");
        }
    }
}
;
