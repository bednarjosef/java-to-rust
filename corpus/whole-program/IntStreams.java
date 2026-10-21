import java.util.stream.IntStream;

// Streams of ints through the JDK's own pipeline code: a range's sum, and a string's chars
// (`String.chars()` streams its UTF-16 code units) filtered, counted, summed and walked.
public class IntStreams {
    public static void main(String[] args) {
        System.out.println(IntStream.range(0, 10).sum());
        System.out.println("Mississippi".chars().filter(c -> c == 's').count());
        String text = "héllo";
        System.out.println(text.chars().sum());
        System.out.println("".chars().count());
        text.chars().forEach(c -> System.out.print(c + " "));
        System.out.println();
    }
}
