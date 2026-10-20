// Features: interfaces with default methods and static interface methods, a class implementing
// two interfaces, an abstract class implementing an interface, super.method() calls to a default
// method (Interface.super.method()), overriding, constants in an interface, instanceof with a
// pattern variable (including two patterns in one condition), a lambda for a functional interface
// with a default method, getClass().getSimpleName(), and for-each over a List of abstract types.
import java.util.ArrayList;
import java.util.List;

public class AnimalsInterfaces {
    interface Named {
        String name();

        default String greeting() {
            return "I am " + name();
        }
    }

    interface Walker {
        int LEGS_DEFAULT = 4;

        default int legs() {
            return LEGS_DEFAULT;
        }

        default String gait() {
            return "walks";
        }

        static String kind() {
            return "walker";
        }
    }

    interface Swimmer {
        default String gait() {
            return "swims";
        }
    }

    static abstract class Animal implements Named {
        private final String name;
        protected int energy;

        Animal(String name, int energy) {
            this.name = name;
            this.energy = energy;
        }

        @Override
        public String name() {
            return name;
        }

        abstract String sound();

        void eat(int food) {
            energy += food;
        }

        @Override
        public String toString() {
            return getClass().getSimpleName() + "(" + name + ", energy " + energy + ")";
        }
    }

    static class Dog extends Animal implements Walker {
        Dog(String name) {
            super(name, 10);
        }

        @Override
        String sound() {
            return "woof";
        }

        @Override
        public String gait() {
            return "trots";
        }

        @Override
        public String greeting() {
            return super.greeting() + " and I am a dog";
        }
    }

    static class Duck extends Animal implements Walker, Swimmer {
        Duck(String name) {
            super(name, 5);
        }

        @Override
        String sound() {
            return "quack";
        }

        @Override
        public int legs() {
            return 2;
        }

        @Override
        public String gait() {
            return Swimmer.super.gait() + " and " + Walker.super.gait();
        }
    }

    static class Fish extends Animal implements Swimmer {
        Fish(String name) {
            super(name, 2);
        }

        @Override
        String sound() {
            return "blub";
        }
    }

    static String describe(Object o) {
        if (o instanceof Walker w && o instanceof Animal a) {
            return a.name() + " has " + w.legs() + " legs and " + w.gait();
        }
        if (o instanceof Swimmer s) {
            return "swimmer using " + s.getClass().getSimpleName();
        }
        return "unknown " + o;
    }

    public static void main(String[] args) {
        List<Animal> zoo = new ArrayList<>();
        zoo.add(new Dog("rex"));
        zoo.add(new Duck("donald"));
        zoo.add(new Fish("nemo"));

        for (Animal a : zoo) {
            System.out.println(a + " says " + a.sound());
            System.out.println("  " + a.greeting());
            System.out.println("  " + describe(a));
        }

        Animal rex = zoo.get(0);
        rex.eat(7);
        System.out.println("after eating: " + rex);
        System.out.println("walker kind: " + Walker.kind());
        System.out.println("default legs constant: " + Walker.LEGS_DEFAULT);

        Named anonymous = () -> "lambda thing";
        System.out.println(anonymous.greeting());
        System.out.println(describe("just a string"));

        int totalLegs = 0;
        for (Animal a : zoo) {
            if (a instanceof Walker walker) {
                totalLegs += walker.legs();
            }
        }
        System.out.println("total legs: " + totalLegs);
    }
}
