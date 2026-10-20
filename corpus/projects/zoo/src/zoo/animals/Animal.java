package zoo.animals;

public abstract class Animal {
    protected final String name;
    protected int meals;

    protected Animal(String name) {
        this.name = name;
    }

    public abstract String sound();

    public int legs() {
        return 4;
    }

    public void feed(int portions) {
        meals += portions;
    }

    public int meals() {
        return meals;
    }

    public String name() {
        return name;
    }

    public String toString() {
        return name + " (" + sound() + ", " + legs() + " legs, " + meals + " meals)";
    }
}
