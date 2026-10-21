// An enum is initialized on the first use of one of its constants, of values() or of valueOf(),
// not when the program starts: what its constructors and static block print comes then.
public class EnumFirstUse {
    enum Level {
        LOW(1), HIGH(10);

        final int weight;

        Level(int weight) {
            System.out.println("building " + name());
            int w = 0;
            for (int i = 0; i < weight; i++) {
                w += 2;
            }
            this.weight = w;
        }
    }

    enum Color {
        RED, GREEN;

        static {
            System.out.println("colors ready");
        }
    }

    enum Size {
        SMALL, LARGE;

        static int made = 0;

        Size() {
            System.out.println("size " + ordinal());
        }
    }

    public static void main(String[] args) {
        System.out.println("start");
        Level level = Level.HIGH;
        System.out.println("got " + level.ordinal());
        System.out.println(level.weight);
        System.out.println("before colors");
        System.out.println(Color.valueOf("GREEN"));
        System.out.println("before sizes");
        System.out.println(Size.values().length);
    }
}
