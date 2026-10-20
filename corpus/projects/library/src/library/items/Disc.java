package library.items;

public class Disc implements Item {
    private final String title;
    private int plays;

    public Disc(String title) {
        this.title = title;
    }

    public String title() {
        return "disc " + title;
    }

    public int days() {
        return 7;
    }

    public void play() {
        plays++;
    }

    public String describe() {
        return title() + " played " + plays + " times";
    }
}
