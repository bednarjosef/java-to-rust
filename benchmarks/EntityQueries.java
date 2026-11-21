// The Minecraft server's entity queries, which collisions, pushing and AI run for most entities
// every tick: Level.getEntities hands the entity getter a consumer lambda, which the getter wraps
// with AbortableIterationConsumer.forConsumer, a static method that only returns a lambda,
// before it passes that to the section storage, which only calls it and lends it on to each
// section. The selector is a predicate made with a default method's lambda, as
// `NO_SPECTATORS.and(..)` makes it for collisions.
import java.util.ArrayList;
import java.util.List;

public class EntityQueries {
    enum Continuation { CONTINUE, ABORT }

    interface Abortable<T> {
        Continuation accept(T entry);

        static <T> Abortable<T> forConsumer(Sink<T> consumer) {
            return e -> {
                consumer.accept(e);
                return Continuation.CONTINUE;
            };
        }
    }

    interface Sink<T> { void accept(T o); }

    static final class Counter implements Sink<Object> {
        int n;
        public void accept(Object o) { n++; }
    }

    interface Selector {
        boolean test(Object o);

        default Selector and(Selector other) { return o -> test(o) && other.test(o); }
    }

    static final class Anything implements Selector {
        public boolean test(Object o) { return o != null; }
    }

    static final class Entity {
        final int id;
        Entity(int id) { this.id = id; }
    }

    static final class Section<T> {
        final Object[] items;
        Section(Object[] items) { this.items = items; }

        @SuppressWarnings("unchecked")
        Continuation sectionEntities(int bound, Abortable<T> output) {
            for (Object item : items) {
                if (((Entity) item).id < bound && output.accept((T) item) == Continuation.ABORT) return Continuation.ABORT;
            }
            return Continuation.CONTINUE;
        }
    }

    static final class Storage<T> {
        final Section<T>[] sections;
        Storage(Section<T>[] sections) { this.sections = sections; }

        void forEachSection(int bound, Abortable<Section<T>> output) {
            for (Section<T> section : sections) {
                if (section != null && output.accept(section) == Continuation.ABORT) return;
            }
        }

        void storageEntities(int bound, Abortable<T> output) {
            forEachSection(bound, section -> section.sectionEntities(bound, output));
        }
    }

    interface EntityGetter<T> { void query(int bound, Sink<T> output); }

    static final class Getter<T> implements EntityGetter<T> {
        private final Storage<T> storage;
        Getter(Storage<T> storage) { this.storage = storage; }
        public void query(int bound, Sink<T> output) {
            this.storage.storageEntities(bound, Abortable.forConsumer(output));
        }
    }

    static EntityGetter<Object> getter;
    static final Selector ANYTHING = new Anything();

    static List<Object> entitiesNear(Object except, int bound, Selector selector) {
        List<Object> output = new ArrayList<>();
        getter.query(bound, entity -> {
            if (entity != except && selector.test(entity)) output.add(entity);
        });
        return output;
    }

    public static void main(String[] args) {
        @SuppressWarnings("unchecked")
        Section<Object>[] sections = new Section[8];
        for (int s = 0; s < sections.length; s++) {
            Object[] items = new Object[8];
            for (int i = 0; i < items.length; i++) items[i] = new Entity((s * 8 + i) * 3);
            sections[s] = new Section<>(items);
        }
        getter = new Getter<>(new Storage<>(sections));
        Counter counter = new Counter();
        getter.query(100, counter);
        long found = 0;
        long start = System.nanoTime();
        for (int tick = 0; tick < 2_000_000; tick++) {
            int limit = tick & 127;
            Object except = sections[tick & 7].items[tick & 7];
            found += entitiesNear(except, tick & 255, ANYTHING.and(o -> ((Entity) o).id < limit)).size();
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        System.out.println(found + " " + counter.n);
    }
}
