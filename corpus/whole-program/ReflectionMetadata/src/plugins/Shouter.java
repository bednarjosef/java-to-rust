package plugins;

public class Shouter {
    private static int made;

    public Shouter() {
        made++;
    }

    public static int made() {
        return made;
    }

    public String shout(String who) {
        return who.toUpperCase() + "!";
    }

    public String whisper(String who) {
        return who.toLowerCase() + "...";
    }
}
