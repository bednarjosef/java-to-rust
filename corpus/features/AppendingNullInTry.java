import java.util.function.Consumer;

// Appending or inserting a null string or boxed value writes "null" and throws nothing, also in a
// lambda that runs inside a `try` whose `catch` and `finally` would see a NullPointerException.
public class AppendingNullInTry {
    static final StringBuilder log = new StringBuilder();

    static String name(int i) {
        return i % 2 == 0 ? null : "n" + i;
    }

    public static void main(String[] args) {
        Consumer<String> note = s -> log.append(s).append(' ');
        Runnable done = () -> log.insert(0, name(2)).append('|');
        try {
            for (int i = 0; i < 4; i++) {
                note.accept(name(i));
            }
            done.run();
            Integer boxed = args.length > 5 ? Integer.valueOf(1) : null;
            log.append(boxed).append(new StringBuilder("!"));
            System.out.println(log);
        } catch (NullPointerException e) {
            System.out.println("caught " + e);
        } finally {
            System.out.println("done");
        }
    }
}
