// A broad driver of the JDK's main public API, for the census of what the translator rejects
// (docs/jdk-coverage.md): java.util's collections, maps, Arrays, Collections, Optional, Objects,
// StringJoiner, iterators and the common streams; java.lang's StringBuilder, Math, boxes,
// Character, Integer parsing, enums and records; and java.util.function.
import java.util.*;
import java.util.function.*;
import java.util.stream.*;

public class JdkApiDriver {
    enum Color { RED, GREEN, BLUE }

    record Point(int x, int y) {
        int dist() { return Math.abs(x) + Math.abs(y); }
    }

    public static void main(String[] args) {
        lists();
        sets();
        maps();
        queues();
        arraysAndCollections();
        optionalAndObjects();
        iterators();
        streams();
        functions();
        lang();
        enumsAndRecords();
    }

    static void lists() {
        List<Integer> list = new ArrayList<>();
        for (int i = 0; i < 10; i++) list.add(i * 3 % 7);
        list.add(0, 42);
        list.set(1, 5);
        list.remove(Integer.valueOf(42));
        list.remove(0);
        System.out.println(list + " " + list.size() + " " + list.contains(6) + " " + list.indexOf(6));
        List<Integer> linked = new LinkedList<>(list);
        linked.addAll(List.of(1, 2, 3));
        System.out.println(linked.subList(2, 5) + " " + linked.lastIndexOf(1) + " " + linked.isEmpty());
        list.removeIf(x -> x % 2 == 0);
        list.replaceAll(x -> x * 10);
        list.sort(Comparator.reverseOrder());
        System.out.println(list + " " + list.equals(new ArrayList<>(list)) + " " + list.hashCode());
        Object[] array = list.toArray();
        Integer[] typed = list.toArray(new Integer[0]);
        System.out.println(array.length + " " + typed.length);
        list.clear();
        System.out.println(list.isEmpty());
    }

    static void sets() {
        Set<String> hash = new HashSet<>(Arrays.asList("b", "a", "c", "a"));
        Set<String> linked = new LinkedHashSet<>(List.of("z", "y", "x"));
        TreeSet<String> tree = new TreeSet<>(hash);
        tree.addAll(linked);
        System.out.println(hash.size() + " " + linked + " " + tree + " " + tree.first() + " " + tree.last());
        System.out.println(tree.headSet("c") + " " + tree.tailSet("x") + " " + tree.ceiling("d") + " " + tree.floor("d"));
        hash.retainAll(Set.of("a", "b"));
        hash.removeAll(Set.of("a"));
        System.out.println(hash + " " + hash.contains("b") + " " + tree.descendingSet() + " " + tree.pollFirst());
        EnumSet<Color> colors = EnumSet.of(Color.RED, Color.BLUE);
        System.out.println(colors + " " + EnumSet.complementOf(colors) + " " + EnumSet.allOf(Color.class).size());
    }

    static void maps() {
        Map<String, Integer> counts = new HashMap<>();
        for (String w : "a b c a b a".split(" ")) counts.merge(w, 1, Integer::sum);
        counts.put("d", 4);
        counts.putIfAbsent("d", 5);
        counts.computeIfAbsent("e", k -> 7);
        counts.computeIfPresent("e", (k, v) -> v + 1);
        counts.compute("f", (k, v) -> v == null ? 1 : v + 1);
        System.out.println(new TreeMap<>(counts) + " " + counts.getOrDefault("z", 0) + " " + counts.containsKey("a")
                + " " + counts.containsValue(4));
        Map<String, List<Integer>> groups = new LinkedHashMap<>();
        groups.computeIfAbsent("odd", k -> new ArrayList<>()).add(1);
        groups.computeIfAbsent("even", k -> new ArrayList<>()).add(2);
        for (Map.Entry<String, List<Integer>> e : groups.entrySet()) System.out.println(e.getKey() + "=" + e.getValue());
        TreeMap<Integer, String> tree = new TreeMap<>(Map.of(3, "c", 1, "a", 2, "b"));
        System.out.println(tree.firstKey() + " " + tree.lastEntry().getValue() + " " + tree.headMap(3) + " "
                + tree.tailMap(2, false) + " " + tree.ceilingKey(2) + " " + tree.descendingMap());
        counts.forEach((k, v) -> System.out.print(k + v + ";"));
        System.out.println();
        counts.remove("a");
        counts.replaceAll((k, v) -> v * 2);
        System.out.println(new TreeMap<>(counts).keySet() + " " + new TreeMap<>(counts).values());
        EnumMap<Color, Integer> byColor = new EnumMap<>(Color.class);
        byColor.put(Color.GREEN, 2);
        byColor.put(Color.RED, 1);
        System.out.println(byColor);
    }

    static void queues() {
        Deque<Integer> deque = new ArrayDeque<>();
        deque.push(1);
        deque.addLast(2);
        deque.offerFirst(0);
        System.out.println(deque + " " + deque.peekFirst() + " " + deque.peekLast() + " " + deque.pop() + " "
                + deque.pollLast() + " " + deque.size());
        PriorityQueue<String> pq = new PriorityQueue<>(Comparator.comparing(String::length).thenComparing(s -> s));
        pq.addAll(List.of("ccc", "a", "bb", "aa"));
        StringBuilder order = new StringBuilder();
        while (!pq.isEmpty()) order.append(pq.poll()).append(' ');
        System.out.println(order.toString().trim());
        Queue<Integer> queue = new LinkedList<>();
        queue.offer(5);
        queue.add(6);
        System.out.println(queue.peek() + " " + queue.remove() + " " + queue.poll() + " " + queue.poll());
    }

    static void arraysAndCollections() {
        int[] numbers = {5, 3, 9, 1, 7};
        Arrays.sort(numbers);
        System.out.println(Arrays.toString(numbers) + " " + Arrays.binarySearch(numbers, 7));
        int[] copy = Arrays.copyOf(numbers, 7);
        Arrays.fill(copy, 5, 7, -1);
        System.out.println(Arrays.toString(copy) + " " + Arrays.equals(numbers, copy) + " " + Arrays.hashCode(numbers));
        String[] words = {"pear", "fig", "apple"};
        Arrays.sort(words, Comparator.comparing(String::length));
        System.out.println(Arrays.toString(words) + " " + Arrays.stream(numbers).sum() + " " + Arrays.asList(words));
        List<Integer> list = new ArrayList<>(List.of(4, 2, 8, 6));
        Collections.sort(list);
        Collections.reverse(list);
        Collections.swap(list, 0, 3);
        System.out.println(list + " " + Collections.max(list) + " " + Collections.min(list) + " "
                + Collections.frequency(list, 2) + " " + Collections.nCopies(3, "x") + " " + Collections.emptyList()
                + " " + Collections.singletonList(1));
        Collections.shuffle(list, new Random(42));
        Collections.addAll(list, 10, 11);
        List<Integer> fixed = Collections.unmodifiableList(list);
        try {
            fixed.add(1);
        } catch (UnsupportedOperationException e) {
            System.out.println("unmodifiable " + fixed.size());
        }
        Collections.sort(list, Collections.reverseOrder());
        System.out.println(list + " " + Collections.binarySearch(List.of(1, 3, 5), 3));
    }

    static void optionalAndObjects() {
        Optional<String> some = Optional.of("value");
        Optional<String> none = Optional.empty();
        System.out.println(some.map(String::toUpperCase).orElse("none") + " " + none.orElseGet(() -> "fallback") + " "
                + some.filter(s -> s.length() > 10).isPresent() + " " + none.isEmpty() + " "
                + Optional.ofNullable((String) null).or(() -> Optional.of("or")).get());
        some.ifPresent(s -> System.out.println("present " + s));
        none.ifPresentOrElse(System.out::println, () -> System.out.println("absent"));
        OptionalInt max = IntStream.of(3, 9, 4).max();
        System.out.println(max.getAsInt() + " " + some.flatMap(s -> Optional.of(s.length())).get());
        String text = null;
        System.out.println(Objects.equals(text, null) + " " + Objects.hash(1, "a") + " " + Objects.toString(text, "dflt")
                + " " + Objects.isNull(text) + " " + Objects.requireNonNullElse(text, "else") + " "
                + Objects.hashCode(text));
        try {
            Objects.requireNonNull(text, "text is null");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        StringJoiner joiner = new StringJoiner(", ", "[", "]");
        joiner.add("a").add("b");
        joiner.setEmptyValue("empty");
        System.out.println(joiner + " " + joiner.length() + " " + String.join("|", List.of("x", "y")));
    }

    static void iterators() {
        List<String> list = new ArrayList<>(List.of("a", "b", "c", "d"));
        Iterator<String> it = list.iterator();
        while (it.hasNext()) {
            if (it.next().equals("b")) it.remove();
        }
        ListIterator<String> li = list.listIterator();
        while (li.hasNext()) {
            int index = li.nextIndex();
            String s = li.next();
            li.set(s + index);
            if (s.equals("c")) li.add("c2");
        }
        while (li.hasPrevious()) System.out.print(li.previous() + " ");
        System.out.println();
        Iterable<Integer> range = () -> IntStream.range(0, 3).iterator();
        for (int i : range) System.out.print(i);
        System.out.println();
        list.forEach(System.out::print);
        System.out.println();
    }

    static void streams() {
        List<String> words = List.of("delta", "alpha", "charlie", "bravo", "alpha", "echo");
        System.out.println(words.stream().filter(w -> w.length() == 5).map(String::toUpperCase).sorted()
                .collect(Collectors.toList()));
        System.out.println(words.stream().distinct().count() + " " + words.stream().anyMatch(w -> w.startsWith("e"))
                + " " + words.stream().allMatch(w -> w.length() > 3) + " " + words.stream().noneMatch(String::isEmpty));
        System.out.println(words.stream().collect(Collectors.joining(",", "<", ">")));
        Map<Integer, Long> byLength = words.stream().collect(Collectors.groupingBy(String::length, TreeMap::new,
                Collectors.counting()));
        System.out.println(byLength);
        Map<String, Integer> lengths = words.stream().distinct().collect(Collectors.toMap(w -> w, String::length));
        System.out.println(new TreeMap<>(lengths));
        System.out.println(IntStream.rangeClosed(1, 5).map(i -> i * i).sum() + " "
                + IntStream.range(0, 10).filter(i -> i % 3 == 0).boxed().collect(Collectors.toList()) + " "
                + words.stream().mapToInt(String::length).max().getAsInt() + " "
                + Stream.of(1, 2, 3, 4).reduce(0, Integer::sum) + " "
                + words.stream().limit(2).skip(1).findFirst().orElse("") + " "
                + IntStream.of(1, 2, 3).average().getAsDouble());
        System.out.println(Stream.iterate(1, x -> x * 2).limit(5).map(String::valueOf).collect(Collectors.joining(" "))
                + " " + words.stream().collect(Collectors.partitioningBy(w -> w.length() > 4)) + " "
                + words.stream().sorted(Comparator.reverseOrder()).toList() + " "
                + Stream.of(List.of(1, 2), List.of(3)).flatMap(List::stream).map(x -> x + 1).collect(Collectors.toSet()));
        words.stream().min(Comparator.naturalOrder()).ifPresent(System.out::println);
    }

    static void functions() {
        Function<Integer, Integer> twice = x -> x * 2;
        Function<Integer, Integer> plusOne = x -> x + 1;
        BiFunction<Integer, Integer, Integer> add = Integer::sum;
        Supplier<List<String>> supplier = ArrayList::new;
        Consumer<String> printer = System.out::println;
        Predicate<String> empty = String::isEmpty;
        UnaryOperator<String> upper = String::toUpperCase;
        BinaryOperator<Integer> max = BinaryOperator.maxBy(Comparator.naturalOrder());
        IntBinaryOperator times = (a, b) -> a * b;
        IntPredicate even = i -> i % 2 == 0;
        ToIntFunction<String> length = String::length;
        System.out.println(twice.andThen(plusOne).apply(5) + " " + twice.compose(plusOne).apply(5) + " "
                + add.andThen(twice).apply(2, 3) + " " + supplier.get().size() + " " + empty.negate().test("x") + " "
                + empty.or(s -> s.startsWith("a")).test("ab") + " " + upper.apply("up") + " " + max.apply(3, 9) + " "
                + times.applyAsInt(6, 7) + " " + even.test(4) + " " + length.applyAsInt("four") + " "
                + Function.<Integer>identity().apply(8) + " " + Predicate.not(empty).test(""));
        printer.andThen(s -> System.out.println(s.length())).accept("consumed");
    }

    static void lang() {
        StringBuilder sb = new StringBuilder("hello");
        sb.append(' ').append(42).append(true).insert(0, ">").reverse().setCharAt(0, 'E');
        sb.deleteCharAt(1).replace(0, 2, "AB");
        System.out.println(sb + " " + sb.length() + " " + sb.indexOf("l") + " " + sb.charAt(3));
        System.out.println(Math.max(3, 7) + " " + Math.floorMod(-7, 3) + " " + Math.sqrt(2.0) + " " + Math.pow(2, 10)
                + " " + Math.round(2.5) + " " + Math.abs(-4.5) + " " + Math.floor(-1.5) + " " + Math.ceil(1.2) + " "
                + Math.hypot(3, 4) + " " + Math.signum(-2.0));
        Integer boxed = Integer.valueOf(127);
        Long big = Long.valueOf(1L << 40);
        Double d = Double.valueOf("3.25");
        System.out.println(boxed.equals(127) + " " + Integer.compare(3, 5) + " " + Integer.parseInt("-123") + " "
                + Integer.parseInt("ff", 16) + " " + Integer.toBinaryString(10) + " " + Integer.toHexString(255) + " "
                + Integer.MAX_VALUE + " " + Long.parseLong("9000000000") + " " + big.intValue() + " " + d.intValue()
                + " " + Double.compare(1.0, 2.0) + " " + Boolean.parseBoolean("TRUE") + " " + Integer.bitCount(255)
                + " " + Integer.valueOf(5).hashCode() + " " + Character.getNumericValue('7'));
        try {
            Integer.parseInt("x1");
        } catch (NumberFormatException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(Character.isDigit('5') + " " + Character.isLetter('x') + " " + Character.toUpperCase('q')
                + " " + Character.isWhitespace(' ') + " " + (char) ('a' + 2) + " " + Character.valueOf('z').compareTo('a'));
        String s = "  Mixed Case  ";
        System.out.println(s.trim().toLowerCase() + "|" + s.strip().replace('e', '3') + "|" + s.isBlank() + "|"
                + "a,b,,c".split(",").length + "|" + "abc".compareTo("abd") + "|" + "x".repeat(3) + "|"
                + String.valueOf(3.5) + "|" + "hello".chars().filter(c -> c == 'l').count());
    }

    static void enumsAndRecords() {
        for (Color c : Color.values()) {
            String kind = switch (c) {
                case RED -> "warm";
                case GREEN, BLUE -> "cool";
            };
            System.out.print(c + ":" + c.ordinal() + ":" + kind + " ");
        }
        System.out.println(Color.valueOf("BLUE").compareTo(Color.RED) + " " + Color.GREEN.name());
        Point p = new Point(3, -4);
        Point q = new Point(3, -4);
        Set<Point> points = new HashSet<>(List.of(p, new Point(0, 0)));
        System.out.println(p + " " + p.equals(q) + " " + (p.hashCode() == q.hashCode()) + " " + p.dist() + " "
                + points.contains(q) + " " + p.x());
        List<Point> sorted = new ArrayList<>(points);
        sorted.sort(Comparator.comparingInt(Point::dist).thenComparing(Point::y));
        System.out.println(sorted);
    }
}
