// The Minecraft server's collector of an entity's block effects: lists of effects kept in
// EnumMaps by kind, moved into one list and cleared at the end of each step, kind by kind. Each
// read of a map's value is a cast of an `Object` (the map's erased value) to `List`, an
// interface that many of the JDK's classes implement, where the lists in the maps are all
// ArrayLists. The program holds lists of other classes as `Object`s too, so the cast's own test
// is the interface's table of classes.
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;

public class EnumMapLists {
    enum Kind {
        FREEZE,
        FIRE_IGNITE,
        LAVA_IGNITE,
        EXTINGUISH,
        CLEAR_FREEZE
    }

    interface Effect {
        int apply(int value);
    }

    static final class Collector {
        final Map<Kind, List<Effect>> before = new EnumMap<>(Kind.class);
        final Map<Kind, List<Effect>> after = new EnumMap<>(Kind.class);
        final Set<Kind> pending = EnumSet.noneOf(Kind.class);
        final List<Effect> finalEffects = new ArrayList<>();

        Collector() {
            for (Kind k : Kind.values()) {
                before.put(k, new ArrayList<>());
                after.put(k, new ArrayList<>());
            }
        }

        void runBefore(Kind kind, Effect effect) {
            before.get(kind).add(effect);
        }

        void flushStep() {
            for (Kind kind : Kind.values()) {
                List<Effect> beforeEffects = before.get(kind);
                finalEffects.addAll(beforeEffects);
                beforeEffects.clear();
                if (pending.remove(kind)) {
                    finalEffects.add(v -> v + kind.ordinal());
                }
                List<Effect> afterEffects = after.get(kind);
                finalEffects.addAll(afterEffects);
                afterEffects.clear();
            }
        }

        int applyAndClear(int value) {
            flushStep();
            for (Effect effect : finalEffects) {
                value = effect.apply(value);
            }
            finalEffects.clear();
            return value;
        }
    }

    static int size(Object o) {
        return o instanceof List<?> l ? l.size() : -1;
    }

    static long run(Collector[] collectors, int rounds) {
        long sum = 0;
        int seed = 12345;
        for (int r = 0; r < rounds; r++) {
            for (Collector c : collectors) {
                for (int step = 0; step < 4; step++) {
                    seed = seed * 1103515245 + 12345;
                    // Most steps add nothing, as most entities touch no block with an effect.
                    if ((seed >>> 24 & 31) == 0) {
                        int add = seed >>> 8 & 7;
                        c.runBefore(Kind.values()[seed >>> 16 & 3], v -> v * 3 + add);
                    }
                    if ((seed >>> 20 & 63) == 0) {
                        c.pending.add(Kind.values()[seed >>> 12 & 3]);
                    }
                    c.flushStep();
                }
                sum += c.applyAndClear(r & 7);
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        // Lists of other classes held as `Object`s, which a cast to `List` must tell apart. They
        // are passed as `Object`s, not stored in an `Object[]`, whose elements the points-to facts
        // pool with every other array of objects, the maps' among them.
        int sizes = size(new LinkedList<Integer>()) + size(new CopyOnWriteArrayList<Integer>())
            + size(Collections.emptyList()) + size(Collections.singletonList(1)) + size(List.of(1, 2))
            + size(List.of()) + size(Collections.unmodifiableList(new ArrayList<>()))
            + size(new ArrayList<Integer>().subList(0, 0)) + size(new ArrayDeque<Integer>());
        Collector[] collectors = new Collector[256];
        for (int i = 0; i < collectors.length; i++) {
            collectors[i] = new Collector();
        }
        long check = run(collectors, 200);
        long start = System.nanoTime();
        long result = run(collectors, 4000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("enum map lists " + result + " " + check + " " + sizes);
        System.err.println("kernel-ms " + elapsed);
    }
}
