
// The messages of NullPointerExceptions where lowering keeps a value in a local of its own: an
// argument computed before its receiver is checked and unboxed where the call reads it, an
// element read once for several reads, the class of getEnumConstants(), a varargs call in a
// chain (with arguments and without), and a lambda's parameter whose name has a `$`, as a
// decompiler writes them. Also the class of isAssignableFrom, and its null argument, which has
// no message.
public class HeldValueMessages {
    static class Key<T> {
        final int id;

        Key(int id) {
            this.id = id;
        }
    }

    static class Data {
        Object[] values = new Object[4];

        @SuppressWarnings("unchecked")
        <T> T get(Key<T> key) {
            return (T) values[key.id];
        }
    }

    static class Output {
        void putByte(String name, byte b) {
            System.out.println(name + b);
        }

        void putLong(String name, long b) {
            System.out.println(name + b);
        }
    }

    interface Handler {}

    static class Flow implements Handler {}

    interface Pipeline {
        Pipeline addLast(String name, Handler handler);

        Pipeline addLast(Handler... handlers);
    }

    static class Ends implements Pipeline {
        public Pipeline addLast(String name, Handler handler) {
            return name.equals("first") ? null : this;
        }

        public Pipeline addLast(Handler... handlers) {
            return handlers.length == 0 ? null : this;
        }
    }

    interface Named {
        String name();
    }

    enum Color {
        RED,
        GREEN
    }

    interface Supplier<T> {
        T get();
    }

    interface ToIntFunction<T> {
        int applyAsInt(T value);
    }

    static class Maybe<T> {
        T value;

        T orElse(T other) {
            return value == null ? other : value;
        }
    }

    static class Part {
        double x, y;
    }

    static final Key<Byte> FLAGS = new Key<>(1);
    static final Key<Long> TICK = new Key<>(2);

    Data entityData = new Data();
    Maybe<Integer> event = new Maybe<>();
    Supplier<Integer> sound = () -> null;
    Class<?> baseClass;
    Part[] parts = new Part[2];

    void save(Output output) {
        output.putByte("Flags", this.entityData.get(FLAGS));
    }

    void saveLong(Output output) {
        output.putLong("Tick", this.entityData.get(TICK));
    }

    void dispense(Output output) {
        output.putLong("Event", this.event.orElse(null));
    }

    void convert(Output output) {
        output.putLong("Sound", this.sound.get());
    }

    boolean assignable(Class<?> index) {
        return this.baseClass.isAssignableFrom(index);
    }

    static Object constant(Class<?> type, int i) {
        return type.getEnumConstants()[i];
    }

    void move(Part[] old) {
        for (int i = 0; i < this.parts.length; i++) {
            this.parts[i].x = i;
            this.parts[i].y = old[i].y;
        }
    }

    void moveLater(Part[] old) {
        for (int i = 0; i < old.length; i++) {
            old[i].x = i;
            old[i].y = this.parts[i].y + old[i].x;
        }
    }

    static void configure(Pipeline pipeline, String first) {
        pipeline.addLast(first, new Flow()).addLast(new Flow()).addLast("last", new Flow());
    }

    static void configureEmpty(Pipeline pipeline) {
        pipeline.addLast("second", new Flow()).addLast().addLast("last", new Flow());
    }

    static int names(Named[] all) {
        int total = 0;
        for (Named n : all) {
            ToIntFunction<Object> length = rec$ -> ((Named) rec$).name().length();
            total += length.applyAsInt(n);
        }
        return total;
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        HeldValueMessages m = new HeldValueMessages();
        Output o = new Output();
        report(() -> m.save(o));
        report(() -> m.saveLong(o));
        report(() -> m.dispense(o));
        report(() -> m.convert(o));
        report(() -> m.assignable(String.class));
        m.baseClass = Object.class;
        report(() -> System.out.println(m.assignable(String.class)));
        report(() -> m.assignable(null));
        report(() -> System.out.println(constant(Color.class, 1)));
        report(() -> constant(String.class, 0));
        report(() -> constant(null, 0));
        report(() -> m.move(new Part[2]));
        m.parts[0] = new Part();
        m.parts[1] = new Part();
        Part[] old = { new Part(), new Part() };
        report(() -> m.move(old));
        report(() -> m.moveLater(old));
        m.parts[1] = null;
        report(() -> m.moveLater(old));
        report(() -> configure(new Ends(), "first"));
        report(() -> configure(new Ends(), "second"));
        report(() -> configure(null, "first"));
        report(() -> configureEmpty(new Ends()));
        report(() -> System.out.println(names(new Named[] {() -> "one", () -> "three"})));
        report(() -> names(new Named[] {null}));
        report(() -> names(new Named[] {() -> null}));
    }
}
