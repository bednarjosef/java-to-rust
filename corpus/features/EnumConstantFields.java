// Fields in the bodies of enum constants: each body declares its own fields, with constant initial
// values, which its methods read. The constants whose body does not declare a field have none.
public class EnumConstantFields {
    enum Planet {
        MERCURY(1) {
            int orbitDays = 88;

            String describe() {
                return "small, orbit of " + orbitDays + " days";
            }
        },
        EARTH(3) {
            int moons = 1;
            boolean habitable = true;

            String describe() {
                return "home with " + moons + " moon" + (habitable ? "" : "?");
            }
        },
        MARS(2) {
            long seconds = 88775244L;
            double gravity = 3.71;
            char symbol = 'M';

            String describe() {
                return "red, " + gravity + " m/s2, " + symbol + " " + seconds;
            }
        },
        PLUTO(0) {
            int twice = 2 * 21;

            String describe() {
                return "dwarf, " + twice;
            }
        };

        private final int rank;

        Planet(int rank) {
            this.rank = rank;
        }

        abstract String describe();

        String summary() {
            return name() + " #" + rank + ": " + describe();
        }
    }

    public static void main(String[] args) {
        for (Planet p : Planet.values()) {
            System.out.println(p.summary());
        }
        System.out.println(Planet.EARTH.describe().length());
    }
}
