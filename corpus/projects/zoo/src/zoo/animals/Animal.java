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

    // Two animals are equal when they have the same name and make the same sound.
    @Override
    public boolean equals(Object o) {
        return o instanceof Animal other && other.name.equals(name) && other.sound().equals(sound());
    }

    @Override
    public int hashCode() {
        return name.hashCode() * 31 + legs();
    }

    public String toString() {
        return name + " (" + sound() + ", " + legs() + " legs, " + meals + " meals)";
    }
}
