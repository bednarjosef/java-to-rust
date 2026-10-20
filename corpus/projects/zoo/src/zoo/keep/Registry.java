package zoo.keep;

import zoo.animals.Animal;
import zoo.animals.Mammal;

public class Registry {
    private final Animal[] animals = new Animal[8];
    private int count;
    private Animal hungriest;

    public void add(Animal a) {
        animals[count] = a;
        count++;
    }

    public void feedAll(int portions) {
        for (int i = 0; i < count; i++) {
            animals[i].feed(portions);
        }
    }

    public Animal hungriest() {
        hungriest = null;
        for (int i = 0; i < count; i++) {
            if (hungriest == null || animals[i].meals() < hungriest.meals()) {
                hungriest = animals[i];
            }
        }
        return hungriest;
    }

    public int legs() {
        int total = 0;
        for (int i = 0; i < count; i++) {
            total += animals[i].legs();
        }
        return total;
    }

    public int furry(Mammal[] mammals) {
        int n = 0;
        for (Mammal m : mammals) {
            if (m.isFurry()) {
                n++;
            }
        }
        return n;
    }

    public void print() {
        for (int i = 0; i < count; i++) {
            System.out.println("  " + animals[i]);
        }
    }
}
