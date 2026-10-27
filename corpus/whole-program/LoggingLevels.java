import java.util.logging.Level;

// The levels of `java.util.logging`, which `Level`'s class keeps by name and by value as each is
// made, its custom ones with their class loader: parsing a level's name or number finds a known
// one, and a number of no known level makes a new one.
public class LoggingLevels {
    public static void main(String[] args) {
        Level[] levels = {Level.SEVERE, Level.WARNING, Level.INFO, Level.CONFIG, Level.FINE, Level.ALL, Level.OFF};
        for (Level level : levels) {
            System.out.println(level.getName() + " " + level.intValue() + " " + level);
        }
        System.out.println(Level.parse("INFO") == Level.INFO);
        System.out.println(Level.parse("900") == Level.WARNING);
        Level odd = Level.parse("850");
        System.out.println(odd.getName() + " " + odd.intValue() + " " + (Level.parse("850") == odd));
        try {
            Level.parse("LOUD");
        } catch (IllegalArgumentException e) {
            System.out.println("rejected: " + e.getMessage());
        }
        System.out.println(Level.INFO.intValue() > Level.FINE.intValue());
        Level loud = new Loud();
        System.out.println(Level.parse("LOUD") == loud);
        System.out.println(Level.parse("1100").getName() + " " + Level.parse("LOUD").intValue());
    }

    static final class Loud extends Level {
        Loud() {
            super("LOUD", 1100);
        }
    }
}
