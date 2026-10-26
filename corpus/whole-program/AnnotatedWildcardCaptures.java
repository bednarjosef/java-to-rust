import java.io.IOException;
import java.io.StringWriter;
import java.io.Writer;
import java.lang.annotation.ElementType;
import java.lang.annotation.Target;
import java.util.List;
import java.util.function.Function;
import java.util.stream.Collectors;
import java.util.stream.Stream;

// A type annotation on the bound of a wildcard, as JSpecify's `@Nullable` in the Minecraft
// server's `CsvOutput.writeLine(Stream<? extends @Nullable Object>)`: the values read through
// the wildcard are of a captured type variable, which javac prints with the annotation.
public class AnnotatedWildcardCaptures {
    @Target(ElementType.TYPE_USE)
    @interface Nullable {}

    private final Writer output;

    AnnotatedWildcardCaptures(Writer output) {
        this.output = output;
    }

    void writeRow(final @Nullable Object... values) throws IOException {
        this.writeLine(Stream.of(values));
    }

    private void writeLine(final Stream<? extends @Nullable Object> values) throws IOException {
        this.output.write(values.map(AnnotatedWildcardCaptures::text).collect(Collectors.joining(",")) + "\n");
    }

    private static String text(final @Nullable Object value) {
        return value != null ? value.toString() : "[null]";
    }

    static int total(List<? extends @Nullable Number> numbers, Function<? super @Nullable Number, Integer> f) {
        int sum = 0;
        for (Number n : numbers) {
            sum += f.apply(n);
        }
        return sum;
    }

    public static void main(String[] args) throws IOException {
        StringWriter w = new StringWriter();
        AnnotatedWildcardCaptures csv = new AnnotatedWildcardCaptures(w);
        csv.writeRow("a", 1, null);
        csv.writeRow(2.5, "b", 'c');
        System.out.print(w);
        System.out.println(total(List.of(1, 2.5, 3L), n -> n == null ? 0 : n.intValue()));
    }
}
