package zoo.animals;

public class Whale extends Mammal {
    public Whale(String name) {
        super(name, false);
    }

    public String sound() {
        return "~~~";
    }

    public int legs() {
        return 0;
    }

    public void feed(int portions) {
        super.feed(portions * 100);
    }
}
