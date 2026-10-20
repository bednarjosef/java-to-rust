public class Dispatch {
    static class Animal {
        String name;
        int calls;
        Animal(String name) { this.name = name; }
        String sound() { return "..."; }
        String speak() { calls++; return name + " says " + sound(); }
        int legs() { return 4; }
        String kind() { return "animal"; }
        public String toString() { return kind() + " " + name; }
    }

    static class Dog extends Animal {
        Dog(String name) { super(name); }
        String sound() { return "woof"; }
        String kind() { return "dog"; }
    }

    static class Puppy extends Dog {
        int age;
        Puppy(String name, int age) { super(name); this.age = age; }
        String sound() { return "yip (" + super.sound() + ")"; }
        String kind() { return "puppy of " + age + ", a " + super.kind(); }
    }

    static class Bird extends Animal {
        Bird(String name) { super(name); }
        int legs() { return 2; }
        String sound() { return "tweet"; }
        String speak() { return "the bird: " + super.speak(); }
    }

    static abstract class Vehicle {
        abstract int wheels();
        int axles() { return wheels() / 2; }
        String describe() { return wheels() + " wheels, " + axles() + " axles"; }
    }

    static class Car extends Vehicle {
        int wheels() { return 4; }
    }

    static class Truck extends Vehicle {
        int trailers;
        Truck(int trailers) { this.trailers = trailers; }
        int wheels() { return 6 + 4 * trailers; }
        String describe() { return "truck: " + super.describe(); }
    }

    static int totalLegs(Animal[] animals) {
        int total = 0;
        for (Animal a : animals) {
            total += a.legs();
        }
        return total;
    }

    static Animal loudest(Animal a, Animal b) {
        return a.speak().length() >= b.speak().length() ? a : b;
    }

    public static void main(String[] args) {
        Animal[] zoo = { new Animal("generic"), new Dog("rex"), new Puppy("bit", 1), new Bird("tweety") };
        for (Animal a : zoo) {
            System.out.println(a.speak());
            System.out.println(a);
        }
        System.out.println(totalLegs(zoo));
        System.out.println(loudest(zoo[1], zoo[3]));
        int calls = 0;
        for (Animal a : zoo) {
            calls += a.calls;
        }
        System.out.println("calls " + calls);
        Dog d = new Puppy("small", 2);
        System.out.println(d.sound() + " " + d.kind() + " " + d.legs());
        Animal plain = d;
        System.out.println(plain.speak());
        Vehicle[] fleet = { new Car(), new Truck(0), new Truck(2) };
        for (Vehicle v : fleet) {
            System.out.println(v.describe());
        }
        Truck t = new Truck(1);
        t.trailers++;
        Vehicle v = t;
        System.out.println(v.wheels() + " " + t.axles());
    }
}
