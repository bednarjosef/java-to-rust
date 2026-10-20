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
    }
}
