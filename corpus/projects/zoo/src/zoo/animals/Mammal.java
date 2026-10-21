package zoo.animals;

public abstract class Mammal extends Animal {
    private final boolean furry;

    protected Mammal(String name, boolean furry) {
        super(name);
        this.furry = furry;
    }

    public boolean isFurry() {
        return furry;
    }

    public String toString() {
        return (furry ? "furry " : "") + super.toString();
    }
}
