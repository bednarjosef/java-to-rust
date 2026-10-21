import java.util.EnumMap;
import java.util.EnumSet;
import java.util.Map;

// EnumMap and EnumSet, translated from the JDK's sources, and an enum whose constructor builds an
// EnumMap, as the JDK's StreamOpFlag does.
public class EnumCollections {
    enum Day { MONDAY, TUESDAY, WEDNESDAY, THURSDAY, FRIDAY, SATURDAY, SUNDAY }

    enum Kind { SOURCE, OPERATION, TERMINAL }

    enum Flag {
        SIZED(0, set(Kind.SOURCE).set(Kind.OPERATION)),
        ORDERED(1, set(Kind.SOURCE).clear(Kind.TERMINAL)),
        DISTINCT(2, set(Kind.OPERATION));

        private final Map<Kind, Integer> masks;
        private final int bit;

        Flag(int position, Masks masks) {
            this.masks = masks.build();
            this.bit = 1 << position;
        }

        private static Masks set(Kind k) {
            return new Masks(new EnumMap<>(Kind.class)).set(k);
        }

        int mask(Kind k) {
            return masks.get(k) * bit;
        }
    }

    private static class Masks {
        final Map<Kind, Integer> map;

        Masks(Map<Kind, Integer> map) {
            this.map = map;
        }

        Masks set(Kind k) {
            map.put(k, 1);
            return this;
        }

        Masks clear(Kind k) {
            map.put(k, 2);
            return this;
        }

        Map<Kind, Integer> build() {
            for (Kind k : Kind.values()) {
                map.putIfAbsent(k, 0);
            }
            return map;
        }
    }

    public static void main(String[] args) {
        EnumMap<Day, Integer> hours = new EnumMap<>(Day.class);
        hours.put(Day.WEDNESDAY, 6);
        hours.put(Day.MONDAY, 8);
        hours.put(Day.FRIDAY, 4);
        hours.put(Day.MONDAY, 9);
        System.out.println(hours.size() + " " + hours.get(Day.MONDAY) + " " + hours.get(Day.SUNDAY));
        System.out.println(hours.containsKey(Day.FRIDAY) + " " + hours.containsKey(Day.TUESDAY));
        int total = 0;
        for (Map.Entry<Day, Integer> e : hours.entrySet()) {
            System.out.println(e.getKey() + " " + e.getValue());
            total += e.getValue();
        }
        System.out.println(total + " " + hours);
        hours.remove(Day.WEDNESDAY);
        System.out.println(hours.keySet() + " " + hours.values());

        EnumSet<Day> weekend = EnumSet.of(Day.SATURDAY, Day.SUNDAY);
        EnumSet<Day> weekdays = EnumSet.complementOf(weekend);
        System.out.println(weekend + " " + weekdays + " " + weekdays.size());
        System.out.println(weekend.contains(Day.SUNDAY) + " " + weekdays.contains(Day.SUNDAY));
        EnumSet<Kind> kinds = EnumSet.noneOf(Kind.class);
        kinds.add(Kind.TERMINAL);
        kinds.add(Kind.SOURCE);
        System.out.println(kinds + " " + EnumSet.allOf(Kind.class) + " " + EnumSet.range(Day.TUESDAY, Day.THURSDAY));
        for (Day d : EnumSet.range(Day.THURSDAY, Day.SATURDAY)) {
            System.out.print(d.ordinal() + " ");
        }
        System.out.println();

        for (Flag f : Flag.values()) {
            System.out.println(f + " " + f.masks + " " + f.mask(Kind.SOURCE) + " " + f.mask(Kind.TERMINAL));
        }
    }
}
