// Class values held in fields of objects that threads share, swapped and compared while other
// threads read them, and a table keyed by an enum's constants that checks a key's class as
// EnumMap does, which threads look keys up in.
public class ClassFieldsAcrossThreads {
    enum Shade { LIGHT, DARK, NONE }
    interface Shape { }
    static final class Circle implements Shape { }
    static final class Square implements Shape { }

    static final class ByShade {
        final Class<?> keyType;
        final Object[] values = new Object[3];

        ByShade(Class<?> keyType) {
            this.keyType = keyType;
        }

        boolean isValidKey(Object key) {
            if (key == null) {
                return false;
            }
            Class<?> keyClass = key.getClass();
            return keyClass == keyType || keyClass.getSuperclass() == keyType;
        }

        Object get(Object key) {
            return isValidKey(key) ? values[((Shade) key).ordinal()] : null;
        }

        void put(Shade key, Object value) {
            values[key.ordinal()] = value;
        }

        boolean containsKey(Object key) {
            return isValidKey(key) && values[((Shade) key).ordinal()] != null;
        }

        public String toString() {
            StringBuilder b = new StringBuilder("{");
            for (Shade s : Shade.values()) {
                b.append(s).append('=').append(values[s.ordinal()]).append(s == Shade.NONE ? "}" : ", ");
            }
            return b.toString();
        }
    }

    static final class Slot {
        Class<?> kind = Circle.class;
        Class<?>[] kinds = { Circle.class, Square.class, int[].class, String.class };
        volatile int swaps;
    }

    public static void main(String[] args) throws InterruptedException {
        Slot slot = new Slot();
        ByShade counts = new ByShade(Shade.class);
        for (Shade s : Shade.values()) {
            counts.put(s, s.ordinal() * 10);
        }
        Thread[] threads = new Thread[4];
        int[] seen = new int[4];
        for (int t = 0; t < 4; t++) {
            int id = t;
            Thread thread = new Thread(() -> {
                int circles = 0;
                for (int i = 0; i < 20000; i++) {
                    Class<?> k = slot.kind;
                    if (k == Circle.class) {
                        circles++;
                    } else if (k != Square.class) {
                        throw new IllegalStateException("torn class " + k);
                    }
                    if (id == 0 && i % 100 == 0) {
                        slot.kind = slot.kind == Circle.class ? Square.class : Circle.class;
                        slot.swaps++;
                    }
                    Shade shade = Shade.values()[i % 3];
                    if ((Integer) counts.get(shade) != shade.ordinal() * 10 || !counts.containsKey(shade)) {
                        throw new IllegalStateException("lost " + shade);
                    }
                }
                synchronized (seen) {
                    seen[id] = circles > 0 ? 1 : 0;
                }
            });
            threads[t] = thread;
            thread.start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        System.out.println("swaps " + slot.swaps + " last " + slot.kind.getSimpleName());
        StringBuilder names = new StringBuilder();
        for (Class<?> k : slot.kinds) {
            names.append(k.getName()).append(' ').append(Shape.class.isAssignableFrom(k)).append(' ');
        }
        System.out.println(names.toString().trim());
        System.out.println(slot.kinds[2].getComponentType() + " " + slot.kinds[0].getSuperclass().getName()
            + " " + (slot.kinds[1] == Square.class) + " " + slot.kinds[1].equals(slot.kind));
        System.out.println(counts + " " + counts.get(null) + " " + counts.containsKey("LIGHT"));
        int any = 0;
        for (int s : seen) {
            any += s;
        }
        System.out.println(any >= 1);
    }
}
