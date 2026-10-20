enum Planet {
    MERCURY(0.33, 2.44),
    EARTH(5.97, 6.37),
    JUPITER(1898.0, 71.49);

    private final double mass;
    private final double radius;

    Planet(double mass, double radius) {
        this.mass = mass;
        this.radius = radius;
    }

    double surfaceGravity() {
        return mass / (radius * radius);
    }

    double scaledMass(double factor) {
        return mass * factor;
    }
}

public class Planets {
    static double total(Planet[] planets) {
        double sum = 0.0;
        for (Planet p : planets) {
            sum += p.scaledMass(2.0);
        }
        return sum;
    }

    static Planet heaviest(Planet a, Planet b) {
        return a.scaledMass(1.0) > b.scaledMass(1.0) ? a : b;
    }

    public static void main(String[] args) {
        Planet[] planets = { Planet.MERCURY, Planet.EARTH, Planet.JUPITER };
        for (Planet p : planets) {
            System.out.println(p + " " + p.ordinal() + " " + p.surfaceGravity());
        }
        System.out.println(total(planets));
        System.out.println(heaviest(Planet.EARTH, Planet.MERCURY));
        Planet named = Planet.valueOf("JUPITER");
        System.out.println(named.compareTo(Planet.EARTH) > 0);
        System.out.println(Planet.valueOf("PLUTO"));
    }
}
