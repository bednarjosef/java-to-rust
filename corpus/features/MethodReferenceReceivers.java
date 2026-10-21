import java.util.function.Function;
import java.util.function.IntBinaryOperator;
import java.util.function.Supplier;

public class MethodReferenceReceivers {
    interface Shape { int area(); }

    static class Counter {
        int count;
        String label;
        Counter(String label) { this.label = label; }
        int next() { return ++count; }
        int add(int a, int b) { return a + b + count; }
        String tag(String s) { return label + ":" + s; }
    }

    static int made = 0;
    Counter current = new Counter("field");

    static Counter make(String label) {
        made++;
        System.out.println("make " + label);
        return new Counter(label);
    }

    void run() {
        Supplier<Integer> next = current::next;
        current = new Counter("other");
        System.out.println(next.get() + " " + next.get() + " " + current.count);
        Function<String, String> tagger = make("made")::tag;
        System.out.println("made " + made);
        System.out.println(tagger.apply("x") + " " + tagger.apply("y") + " made " + made);
        IntBinaryOperator adder = (made > 0 ? make("cond") : current)::add;
        System.out.println(adder.applyAsInt(2, 3));
        Counter[] counters = { new Counter("a"), new Counter("b") };
        int i = 0;
        Supplier<Integer> second = counters[++i]::next;
        i = 0;
        System.out.println(second.get() + " " + counters[1].count + " " + i);
        Function<Integer, Supplier<Integer>> nth = k -> counters[k]::next;
        Supplier<Integer> first = nth.apply(0);
        System.out.println(first.get() + " " + first.get() + " " + nth.apply(1).get());
        Shape shape = new Counter("s")::next;
        System.out.println(shape.area() + shape.area());
        current = null;
        try {
            Supplier<Integer> broken = current::next;
            System.out.println("not reached " + broken);
        } catch (NullPointerException e) {
            System.out.println("null receiver: " + e.getClass().getName());
        }
    }

    public static void main(String[] args) {
        new MethodReferenceReceivers().run();
    }
}
