package zoo.animals;

public class Dog extends Mammal {
    private int tricks;

    public Dog(String name) {
        super(name, true);
    }

    public String sound() {
        return tricks > 1 ? "WOOF" : "woof";
    }

    public void learn() {
        tricks++;
        feed(1);
    }
}
