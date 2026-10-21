// A switch whose cases fall into the next and that has a `default`: every way through it assigns
// the local that is read after it, which Java's definite assignment sees and Rust must too.
public class FallThroughAssignsBeforeUse {
    enum Level { LOW, MID, HIGH, TOP }

    static String band(Level level) {
        String band;
        int weight;
        switch (level) {
            case LOW:
                band = "low";
                weight = 1;
                break;
            case MID:
                band = "mid";
            case HIGH:
                band = "high";
            default:
                band = band(level.ordinal());
                weight = 3;
        }
        return band + weight;
    }

    static String band(int n) {
        String s;
        switch (n % 3) {
            case 0:
                s = "zero";
            case 1:
                s = "one";
                break;
            default:
                s = "many";
        }
        return s;
    }

    public static void main(String[] args) {
        for (Level level : Level.values()) {
            System.out.print(band(level) + " ");
        }
        System.out.println(band(3) + " " + band(5));
    }
}
