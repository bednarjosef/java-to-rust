package util;

import java.util.function.Function;
import marks.CanIgnoreReturnValue;
import marks.DoNotMock;
import marks.Level;
import marks.Nullness;
import marks.Support;
import marks.Weak;
import static marks.Level.FULL;

/**
 * A counter whose code is marked with annotations of a library that the program does not ship:
 * the JVM ignores them, so it runs as if they were not there.
 */
@DoNotMock("Use Counter.create")
@Support(value = Support.Kind.ALL, level = FULL)
public final class Counter<@Nullness T> {
    @Weak
    private final @Nullness Object[] seen = new Object[8];
    private @Nullness String label;
    @marks.Weak private int count;

    private Counter(@Nullness String label) {
        this.label = label;
    }

    public static <@Nullness E> Counter<E> create(String label) {
        return new Counter<>(label);
    }

    @CanIgnoreReturnValue
    public Counter<T> add(@Nullness T value) {
        @Weak int before = count;
        seen[count] = value;
        count = before + 1;
        return this;
    }

    @marks.CanIgnoreReturnValue
    @Support(level = Level.NONE)
    public int count() {
        Function<@Nullness T, String> show = (@Nullness T v) -> String.valueOf(v);
        StringBuilder text = new StringBuilder();
        for (@Nullness Object t : seen) {
            if (t == null) {
                break;
            }
            text.append(show.apply((@Nullness T) t)).append(' ');
        }
        label = label + ": " + text.toString().trim();
        return count;
    }

    public @Nullness String label() {
        return label;
    }

    @Override
    public String toString() {
        return label;
    }

    @DoNotMock
    public interface Visitor {
        @CanIgnoreReturnValue
        boolean visit(@Nullness Object o) throws @Nullness RuntimeException;
    }

    public record Pair(@Nullness String first, @Weak int second) {}

    public enum Mode {
        @Weak FAST,
        SLOW
    }
}
