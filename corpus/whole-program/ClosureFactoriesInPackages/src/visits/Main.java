package visits;

import java.util.ArrayList;
import java.util.List;
import visits.walk.Visitors;

public class Main {
    private final List<Integer> seen = new ArrayList<>();

    void collect(int n) {
        Visitors.Sink sink = i -> seen.add(i);
        Visitors.walk(n, Visitors.forSink(sink));
    }

    public static void main(String[] args) {
        Main main = new Main();
        main.collect(3);
        System.out.println("collected " + main.seen);
        System.out.println("walked " + Visitors.walk(10, Visitors.until(6)));
    }
}
