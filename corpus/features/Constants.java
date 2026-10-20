public class Constants {
    static final int LIMIT = 10;
    static final int DOUBLED = LIMIT * 2;
    static final long BIG = 1L << 40;
    static final double RATE = 0.25;
    static final float HALF = 0.5f;
    static final boolean DEBUG = false;
    static final char OPEN = '(';
    static final byte SMALL = 7;
    static final short MID = 300;
    static final String NAME = "constants";
    static final String GREETING = NAME + " demo";

    public static void main(String[] args) {
        int total = 0;
        for (int i = 0; i < LIMIT; i++) {
            total += i;
        }
        System.out.println(total + " " + DOUBLED + " " + BIG);
        System.out.println(RATE * 2 + " " + HALF + " " + DEBUG);
        System.out.println(OPEN + NAME + SMALL + MID);
        System.out.println(GREETING + " " + GREETING.length());
        String copy = NAME;
        copy = copy + "!";
        System.out.println(copy + " " + NAME);
        System.out.println(SMALL + MID);
    }
}
