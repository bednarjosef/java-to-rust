import java.util.stream.IntStream;

// Streams of ints through the JDK's own pipeline code: a range's sum, and a string's chars
// (`String.chars()` streams its UTF-16 code units) filtered, counted, summed and walked; and
// reductions with `Integer::sum`, as TheAlgorithms' `SumOfDigits.sumOfDigitsFast` writes one.
public class IntStreams {
    public static void main(String[] args) {
        System.out.println(IntStream.range(0, 10).sum());
        System.out.println("Mississippi".chars().filter(c -> c == 's').count());
        String text = "héllo";
        System.out.println(text.chars().sum());
        System.out.println("".chars().count());
        text.chars().forEach(c -> System.out.print(c + " "));
        System.out.println();
        System.out.println(String.valueOf(Math.abs(-98765)).chars().map(c -> c - '0').reduce(0, Integer::sum));
        System.out.println(IntStream.rangeClosed(1, 5).map(i -> i * i).reduce(0, Integer::sum));
    }
}
