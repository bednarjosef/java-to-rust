// An array of `Enum<?>`, which holds the constants of different enums, as the JDK's `EnumSet`
// holds its universe: indexed, walked and compared by ordinal.
public class ArraysOfEnums {
    enum Color { RED, GREEN, BLUE }

    enum Size { SMALL, LARGE }

    static Enum<?> pick(Enum<?>[] all, int i) {
        return all[i % all.length];
    }

    public static void main(String[] args) {
        Enum<?>[] all = {Color.GREEN, Size.LARGE, Color.BLUE, null};
        for (int i = 0; i < 3; i++) {
            Enum<?> e = pick(all, i);
            System.out.println(e.name() + " " + e.ordinal());
        }
        System.out.println(all[3] == null);
        all[3] = Size.SMALL;
        System.out.println(all[3].name() + " " + all.length);
    }
}
