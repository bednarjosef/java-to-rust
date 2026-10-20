// Enums with more in them: fields with initializers (a constant per enum, unless the constructor
// sets the field), fields that are not final but only the constructor sets, a method with the
// same name as a field, and an enum's own toString, which printing and concatenation use while
// name() stays the constant's name.
public class EnumMembers {
    enum Planet {
        MERCURY(3.303e+23, 2.4397e6),
        EARTH(5.976e+24, 6.37814e6),
        JUPITER(1.9e+27, 7.1492e7, true);

        private final double mass;
        private double radius;
        private boolean giant = false;
        private int moons = 0;
        private final String kind = "planet";

        Planet(double mass, double radius) {
            this.mass = mass;
            this.radius = radius;
        }

        Planet(double mass, double radius, boolean giant) {
            this.mass = mass;
            this.radius = radius;
            this.giant = giant;
            this.moons = 95;
        }

        double mass() {
            return mass;
        }

        double radius() {
            return radius;
        }

        double surfaceGravity() {
            return 6.67300E-11 * mass / (radius * radius);
        }

        String describe() {
            return name().toLowerCase() + " (" + kind + (giant ? ", giant" : "") + ", " + moons + " moons)";
        }
    }

    enum Level {
        LOW(1), MEDIUM(5), HIGH(10);

        private final int value;

        Level(int value) {
            this.value = value;
        }

        public String toString() {
            return name().charAt(0) + name().substring(1).toLowerCase() + "=" + value;
        }
    }

    public static void main(String[] args) {
        for (Planet p : Planet.values()) {
            System.out.printf("%s %.2f %.3e %.2f%n", p, p.mass(), p.radius(), p.surfaceGravity());
            System.out.println(p.describe());
        }
        for (Level l : Level.values()) {
            System.out.println(l + " " + l.name() + " " + l.ordinal() + " " + l.toString().length());
        }
        Level high = Level.valueOf("HIGH");
        System.out.println(Planet.valueOf("EARTH").describe());
        System.out.println("level: " + high);
        System.out.println(high);
        System.out.println(String.valueOf(Level.LOW) + String.format(" [%s]", Level.MEDIUM));
    }
}
