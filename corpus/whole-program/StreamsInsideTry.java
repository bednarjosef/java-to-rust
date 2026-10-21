import java.util.List;
import java.util.stream.Collectors;
import java.util.stream.Stream;

// Streams first used inside a `try` with a `finally`, and a flatMap, whose inner streams the JDK
// closes in its own try-with-resources: the static initializers of the JDK's `StreamOpFlag` and
// `Collectors` run there, and they let out nothing, as they run on the JDK's own constants. The
// JDK's try-with-resources adds what closing throws to its exception as suppressed, which reads
// no `NullPointerException`'s message. The catch of `Throwable` does not read the exception: one
// that printed it would need Java's message of each dereference the stream's code may reach.
public class StreamsInsideTry {
    static final StringBuilder log = new StringBuilder();

    public static void main(String[] args) {
        try {
            List<Integer> doubled = Stream.of(1, 2, 3).map(x -> x * 2).collect(Collectors.toList());
            System.out.println(doubled);
        } finally {
            System.out.println("done");
        }
        long total = Stream.of("a", "bb", "ccc")
                .flatMap(s -> Stream.of(s, s.toUpperCase()).onClose(() -> log.append("close ").append(s).append("; ")))
                .peek(s -> log.append(s).append(' '))
                .count();
        System.out.println(total + " | " + log);
        try {
            System.out.println(Stream.of("x", "y", "z").map(String::toUpperCase).collect(Collectors.toList()));
        } catch (Throwable e) {
            System.out.println("caught a throwable");
        }
    }
}
