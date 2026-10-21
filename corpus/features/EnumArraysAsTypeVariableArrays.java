// Arrays of an enum passed for the array of a type variable (`E[]` of `<E extends Enum<E>>`, `T[]`)
// and for an `Enum<?>[]` or an `Object[]`, as a set of enum constants keeps its universe: the
// array is an `Object[]` of the constants, read back as the enum, switched on, stored to, looped
// over and holding a null.
public class EnumArraysAsTypeVariableArrays {
    enum Day { MON, TUE, WED, THU }

    enum Size { S, M }

    static class Flags<E extends Enum<E>> {
        final E[] universe;
        long bits;

        Flags(E[] universe) {
            this.universe = universe;
        }

        void add(E e) {
            bits |= 1L << e.ordinal();
        }

        String show() {
            StringBuilder sb = new StringBuilder("[");
            for (E e : universe) {
                if ((bits & (1L << e.ordinal())) != 0) {
                    sb.append(sb.length() > 1 ? "," : "").append(e.name());
                }
            }
            return sb.append("]").toString();
        }
    }

    static <T> T last(T[] a) {
        return a[a.length - 1];
    }

    static int named(Enum<?>[] universe, String name) {
        for (Enum<?> e : universe) {
            if (e.name().equals(name)) {
                return e.ordinal();
            }
        }
        return -1;
    }

    static int present(Object[] values) {
        int n = 0;
        for (Object v : values) {
            n += v == null ? 0 : 1;
        }
        return n;
    }

    static String kind(Day d) {
        switch (d) {
            case MON:
            case TUE:
                return "early";
            default:
                return "late";
        }
    }

    public static void main(String[] args) {
        Day[] days = Day.values();
        Flags<Day> flags = new Flags<>(days);
        flags.add(Day.WED);
        flags.add(Day.MON);
        System.out.println(flags.show() + " " + last(days) + " " + named(days, "TUE") + " " + named(Size.values(), "M"));
        days[0] = Day.THU;
        System.out.println(days[0] + " " + Day.values()[0] + " " + kind(days[1]) + " " + kind(last(days)));
        Day[] some = new Day[3];
        some[1] = Day.TUE;
        System.out.println(present(some) + " " + (some[0] == null) + " " + some[1].compareTo(Day.WED));
        int total = 0;
        for (Day d : days) {
            total += d.ordinal();
        }
        Flags<Size> sizes = new Flags<>(Size.values());
        sizes.add(Size.M);
        System.out.println(total + " " + sizes.show() + " " + last(Size.values()).name());
    }
}
