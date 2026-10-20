// Features: Stream and IntStream pipelines: filter, map, sorted, distinct, limit, skip, reduce,
// sum, average (OptionalDouble), count, anyMatch, allMatch, findFirst, mapToInt, range, boxed,
// collect with Collectors.toList, toSet, joining, groupingBy into a TreeMap with counting and
// summingInt, partitioningBy, toMap with a merge function, flatMap over nested lists.
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import java.util.stream.Stream;

public class StreamPipeline {
    record Sale(String region, String product, int units, int price) {
        int revenue() {
            return units * price;
        }
    }

    static List<Sale> sales() {
        return List.of(
                new Sale("north", "apple", 12, 3),
                new Sale("south", "pear", 5, 4),
                new Sale("north", "pear", 7, 4),
                new Sale("east", "apple", 20, 2),
                new Sale("south", "plum", 0, 6),
                new Sale("east", "fig", 9, 5),
                new Sale("north", "fig", 3, 5),
                new Sale("west", "apple", 8, 3));
    }

    public static void main(String[] args) {
        List<Sale> all = sales();

        List<String> bigProducts = all.stream()
                .filter(s -> s.units() >= 7)
                .map(Sale::product)
                .distinct()
                .collect(Collectors.toList());
        System.out.println("big products: " + bigProducts);

        int totalRevenue = all.stream().mapToInt(Sale::revenue).sum();
        System.out.println("total revenue: " + totalRevenue);

        Optional<Sale> best = all.stream().max((a, b) -> Integer.compare(a.revenue(), b.revenue()));
        System.out.println("best sale: " + best.map(Sale::region).orElse("none"));

        Map<String, Long> countByRegion = all.stream()
                .collect(Collectors.groupingBy(Sale::region, TreeMap::new, Collectors.counting()));
        System.out.println("sales per region: " + countByRegion);

        Map<String, Integer> unitsByProduct = all.stream()
                .collect(Collectors.groupingBy(Sale::product, TreeMap::new, Collectors.summingInt(Sale::units)));
        System.out.println("units per product: " + unitsByProduct);

        Map<Boolean, List<String>> bigOrSmall = all.stream()
                .collect(Collectors.partitioningBy(s -> s.revenue() >= 40,
                        Collectors.mapping(s -> s.region() + "/" + s.product(), Collectors.toList())));
        System.out.println("big revenue: " + bigOrSmall.get(true));
        System.out.println("small revenue: " + bigOrSmall.get(false));

        String joined = all.stream()
                .map(Sale::product)
                .sorted()
                .skip(2)
                .limit(4)
                .collect(Collectors.joining(", ", "[", "]"));
        System.out.println("joined: " + joined);

        Set<String> regions = all.stream().map(Sale::region).collect(Collectors.toSet());
        System.out.println("regions: " + regions.size() + " contains west: " + regions.contains("west"));

        Map<String, Integer> priceByProduct = all.stream()
                .collect(Collectors.toMap(Sale::product, Sale::price, Math::max, TreeMap::new));
        System.out.println("max price per product: " + priceByProduct);

        IntStream.rangeClosed(1, 5).map(i -> i * i).forEach(i -> System.out.print(i + " "));
        System.out.println();
        int evenSum = IntStream.range(0, 20).filter(i -> i % 2 == 0).sum();
        System.out.println("even sum: " + evenSum);
        double avg = IntStream.of(3, 4, 8).average().orElse(-1);
        System.out.println("average: " + avg);
        System.out.println("empty average: " + IntStream.empty().average().isPresent());

        long count = Stream.iterate(1, x -> x * 3).limit(6).filter(x -> x > 10).count();
        System.out.println("powers of three above 10: " + count);
        System.out.println("reduce product: " + Stream.of(2, 3, 7).reduce(1, (a, b) -> a * b));
        System.out.println("any units zero: " + all.stream().anyMatch(s -> s.units() == 0));
        System.out.println("all prices positive: " + all.stream().allMatch(s -> s.price() > 0));
        System.out.println("first east: " + all.stream().filter(s -> s.region().equals("east")).findFirst()
                .map(Sale::product).orElse("?"));

        List<List<Integer>> nested = List.of(List.of(1, 2), List.of(3), List.of(), List.of(4, 5, 6));
        List<Integer> flat = nested.stream().flatMap(List::stream).map(x -> x * 10).collect(Collectors.toList());
        System.out.println("flat: " + flat);
        List<Integer> boxedSquares = IntStream.range(0, 4).boxed().map(i -> i * i).toList();
        System.out.println("boxed squares: " + boxedSquares);
    }
}
