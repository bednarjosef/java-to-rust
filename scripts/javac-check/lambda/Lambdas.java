import java.util.List;
import java.util.function.Function;
import java.util.function.Supplier;
import java.util.stream.Collectors;

public class Lambdas {
    public static void main(String[] args) {
        int base = args.length + 10;
        Function<Integer, Integer> add = x -> x + base;
        Supplier<String> greeting = () -> "sum";
        Runnable print = () -> System.out.println(greeting.get() + " " + add.apply(5));
        print.run();
        List<String> words = List.of("lambda", "method", "reference");
        System.out.println(words.stream().map(String::length).map(Object::toString).collect(Collectors.joining(",")));
    }
}
