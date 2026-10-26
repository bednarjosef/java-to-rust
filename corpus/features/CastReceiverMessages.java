// The messages of NullPointerExceptions of calls on a cast object that may be null, where the
// cast is to the one class that implements the type cast from (`((ServerLevel) level()).m()`)
// and the method uses its object as a value, so the translation calls it as a function of a
// reference to the object.
public class CastReceiverMessages {
    static abstract class Level {
        abstract int minY();
    }

    static class PoiManager {
        int range() { return 3; }
    }

    static final class ServerLevel extends Level {
        final PoiManager poi = new PoiManager();
        int minY() { return -64; }
        PoiManager getPoiManager() { seen = this; return poi; }
        int logicalHeight() { seen = this; return 384; }
    }

    static Level seen;

    static class Entity {
        Level level;
        Level level() { return level; }
    }

    static class Bee extends Entity {
        class Goal {
            int find() {
                PoiManager m = ((ServerLevel) Bee.this.level()).getPoiManager();
                return m.range();
            }
        }
    }

    static int apply(Level level) {
        return level.minY() + ((ServerLevel) level).logicalHeight() - 1;
    }

    static int height(Level level) {
        return ((ServerLevel) level).logicalHeight();
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        Bee bee = new Bee();
        run(() -> System.out.println(bee.new Goal().find()));
        bee.level = new ServerLevel();
        run(() -> System.out.println(bee.new Goal().find()));
        run(() -> System.out.println(apply(null)));
        run(() -> System.out.println(apply(new ServerLevel())));
        run(() -> System.out.println(height(null)));
        run(() -> System.out.println(height(new ServerLevel()) + " " + (seen != null)));
    }
}
