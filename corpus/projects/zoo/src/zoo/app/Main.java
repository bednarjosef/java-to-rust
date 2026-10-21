package zoo.app;

import zoo.animals.Dog;
import zoo.animals.Mammal;
import zoo.animals.Parrot;
import zoo.animals.Whale;
import zoo.keep.Registry;

public class Main {
    public static void main(String[] args) {
        Registry registry = new Registry();
        Dog rex = new Dog("Rex");
        Whale moby = new Whale("Moby");
        registry.add(rex);
        registry.add(moby);
        registry.add(new Parrot("Polly", "hello"));
        registry.feedAll(1);
        rex.learn();
        rex.learn();
        registry.print();
        System.out.println("legs " + registry.legs());
        System.out.println("hungriest " + registry.hungriest().name());
        Mammal[] mammals = { rex, moby };
        System.out.println("furry " + registry.furry(mammals));
        System.out.println("contains " + registry.contains(new Dog("Rex")) + " " + registry.contains(new Whale("Moby"))
            + " " + registry.contains("Rex") + " " + (rex.hashCode() == new Dog("Rex").hashCode()));
        for (int i = 0; i < 3; i++) {
            Object label = registry.label(i);
            if (label instanceof Integer meals) {
                System.out.println("label " + i + ": " + meals + " meals");
            } else {
                System.out.println("label " + i + ": " + label);
            }
        }
    }
}
