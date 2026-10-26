public class FieldQualifiedInnerCreation {
    static class Log {
        int count = 2;

        class Handler {
            int size() { return count; }
        }
    }

    Log log;
    Log.Handler handler;

    class Round {
        Log.Handler mine;

        Round() { this.mine = log.new Handler(); }
    }

    FieldQualifiedInnerCreation(Log log) { this.log = log; }

    void start() {
        this.handler = log.new Handler();
    }

    public static void main(String[] a) {
        FieldQualifiedInnerCreation f = new FieldQualifiedInnerCreation(new Log());
        try {
            f.start();
            System.out.println(f.handler.size());
            System.out.println(f.new Round().mine.size());
            new FieldQualifiedInnerCreation(null).new Round();
        } catch (NullPointerException e) {
            System.out.println("npe " + e.getMessage());
        }
    }
}
