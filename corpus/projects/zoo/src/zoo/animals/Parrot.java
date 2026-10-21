package zoo.animals;

public class Parrot extends Animal {
    private final String word;

    public Parrot(String name, String word) {
        super(name);
        this.word = word;
    }

    public String sound() {
        return word + "!";
    }

    public int legs() {
        return 2;
    }
}
