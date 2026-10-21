package library.items;

public enum Room implements Item {
    QUIET(1), GROUP(3);

    private final int hours;

    Room(int hours) {
        this.hours = hours;
    }

    public String title() {
        return name().toLowerCase() + " room";
    }

    public int days() {
        return hours;
    }
}
