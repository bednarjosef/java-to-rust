import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;
import java.util.function.Supplier;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

// The messages of nulls that calls of one chain return, where two of its calls have one name
// and start where the chain does, as in javac's `Check.methodsGroupedByName`: the JVM names the
// one that returned the null, by their first arguments, or for calls without arguments, by the
// classes of their receivers (`ThisEscapeAnalyzer.RefSet.find`). And the null that `Class.cast`
// returns, whose call lowering writes as a block.
public class NullMessagesOfChainedCalls {
    static class Node {
        final String label;

        Node(String label) {
            this.label = label;
        }

        Node make(String s) {
            return s == null ? null : new Node(s);
        }

        Node make(int n) {
            return n < 0 ? null : new Node(label + n);
        }

        Node next() {
            return this;
        }
    }

    static class NodeSet extends HashSet<Node> {
        <R extends Node> Stream<R> find(Class<R> type, Predicate<? super R> filter) {
            return stream()
                .filter(type::isInstance)
                .map(type::cast)
                .filter(filter)
                .collect(Collectors.toList())
                .stream();
        }

        @Override
        public Stream<Node> stream() {
            return size() > 1 ? null : super.stream();
        }
    }

    static String chain(Node start, String s, int n) {
        return start.make(s).next().make(n).next().label;
    }

    static String castLabel(Object o) {
        return (Node.class.cast(o)).label;
    }

    static <C extends Collection<Node>> List<C> grouped(Iterable<Object> nodes, Supplier<? extends C> maker) {
        return StreamSupport.stream(nodes.spliterator(), false)
            .map(Node.class::cast)
            .collect(Collectors.groupingBy(node -> node.label, Collectors.toCollection(maker)))
            .entrySet()
            .stream()
            .sorted(Comparator.comparing(e -> e.getKey().toString()))
            .map(Map.Entry::getValue)
            .collect(Collectors.toList());
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        Node start = new Node("n");
        run(() -> System.out.println(chain(start, "a", 1)));
        run(() -> System.out.println(chain(start, null, 1)));
        run(() -> System.out.println(chain(start, "a", -1)));
        run(() -> System.out.println(grouped(List.of(new Node("x"), new Node("y"), new Node("x")), ArrayList::new).size()));
        run(() -> System.out.println(grouped(null, ArrayList::new)));
        run(() -> System.out.println(castLabel(start)));
        run(() -> System.out.println(castLabel(null)));
        NodeSet set = new NodeSet();
        set.add(start);
        run(() -> System.out.println(set.find(Node.class, n -> true).count()));
        set.add(new Node("m"));
        run(() -> System.out.println(set.find(Node.class, n -> true).count()));
    }
}
