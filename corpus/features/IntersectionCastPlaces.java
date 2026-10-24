import java.util.function.Supplier;

// Casts to an intersection type in the places whose type javac's erasure converts the value to
// (a comparison, a string concatenation, a `?:`, a lambda's result, an array's element, a switch
// expression's result, a generic constructor's argument): the bounds are checked in javac's
// order, so that a ClassCastException names the same bound as Java's.
public class IntersectionCastPlaces {
    interface Named {
        String name();
    }

    interface Sized {
        int size();
    }

    static class Base {}

    static class Both extends Base implements Named, Sized {
        public String name() {
            return "both";
        }

        public int size() {
            return 3;
        }

        public String toString() {
            return name();
        }
    }

    static class OnlyNamed implements Named {
        public String name() {
            return "named";
        }
    }

    static class OnlySized implements Sized {
        public int size() {
            return 1;
        }
    }

    static class Box<T> {
        T value;

        Box(T value) {
            this.value = value;
        }
    }

    static boolean same(Object o, Object p) {
        return ((Named & Sized) o) == p;
    }

    static String text(Object o) {
        return "is " + (Sized & Named) o;
    }

    static Object pick(Object o, boolean f) {
        return f ? (Named & Sized) o : "none";
    }

    static Supplier<Named> later(Object o) {
        return () -> (Sized & Named) o;
    }

    static Named[] all(Object o) {
        return new Named[] {(Sized & Named) o};
    }

    static int sized(Object o, int k) {
        Sized s = switch (k) {
            case 1 -> (Named & Sized) o;
            default -> {
                yield (Sized & Named) o;
            }
        };
        return s.size();
    }

    static Box<Named> boxed(Object o) {
        return new Box<Named>((Sized & Named) o);
    }

    static void attempt(String label, Runnable r) {
        try {
            r.run();
        } catch (ClassCastException e) {
            String m = e.getMessage();
            System.out.println(label + ": " + m.substring(0, m.indexOf(" (")));
        }
    }

    public static void main(String[] args) {
        Object both = new Both();
        System.out.println(same(both, both) + " " + text(both) + " " + pick(both, false));
        System.out.println(later(both).get().name() + " " + all(both)[0].name() + " " + sized(both, 1)
                + sized(both, 2) + " " + boxed(both).value.name());
        Object[] others = {new OnlyNamed(), new OnlySized(), "text"};
        for (Object o : others) {
            attempt("same", () -> same(o, null));
            attempt("text", () -> text(o));
            attempt("pick", () -> pick(o, true));
            attempt("later", () -> later(o).get());
            attempt("all", () -> all(o));
            attempt("sized 1", () -> sized(o, 1));
            attempt("sized 2", () -> sized(o, 2));
            attempt("boxed", () -> boxed(o));
        }
    }
}
