// The order in which Java builds an object of a subclass: the superclass's static initialization
// before the subclass's, then the superclass's field initializers and constructor, then the
// subclass's field initializers and constructor body.
public class SuperclassConstructors {
    static int step = 0;

    static int note(String what) {
        step++;
        System.out.println(step + ": " + what);
        return step;
    }

    abstract static class Vehicle {
        static final String KIND = describe("vehicle statics");
        protected final int wheels;
        protected int built = note("vehicle field initializer");
        private String label;

        static String describe(String what) {
            note(what);
            return what;
        }

        Vehicle(int wheels) {
            this(wheels, "unnamed");
        }

        Vehicle(int wheels, String label) {
            note("vehicle constructor with " + wheels + " wheels");
            this.wheels = wheels;
            this.label = label;
        }

        String label() {
            return label;
        }

        void rename(String label) {
            this.label = label;
        }
    }

    static class Bicycle extends Vehicle {
        static final String SOUND = Vehicle.describe("bicycle statics");
        private int gears = note("bicycle field initializer");

        Bicycle() {
            this(3);
            note("bicycle constructor done");
        }

        Bicycle(int gears) {
            super(note("bicycle argument") > 0 ? 2 : 0);
            this.gears = gears;
        }

        int gears() {
            return gears;
        }
    }

    abstract static class Empty {
        Empty() {
        }
    }

    static class Full extends Empty {
        int value = 4;
    }

    public static void main(String[] args) {
        note("main starts");
        Bicycle b = new Bicycle();
        System.out.println(b.label() + " " + b.wheels + " " + b.gears() + " " + b.built);
        b.rename("racer");
        System.out.println(b.label() + " " + Bicycle.SOUND + " " + Vehicle.KIND);
        Bicycle c = new Bicycle(7);
        System.out.println(c.gears() + " " + c.built);
        System.out.println(new Full().value);
    }
}
