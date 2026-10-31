// Methods that return an int, a boolean, a float, a char, a short or a byte and may throw, called
// from other classes and caught there: split into crates, each returns its value widened and its
// caller narrows it back (`java::Ret`), the extremes of each type included.
import java.util.ArrayList;
import java.util.List;

public class SmallResultsAcrossCrates {
    static final class Checked {
        private final int limit;

        Checked(int limit) {
            this.limit = limit;
        }

        int index(int i) {
            if (i < -limit || i > limit) {
                throw new IndexOutOfBoundsException("index " + i + " past " + limit);
            }
            return i * 1_000_003;
        }

        boolean odd(int i) {
            if (i == 13) {
                throw new IllegalArgumentException("unlucky");
            }
            return (index(i) & 1) != 0;
        }

        float ratio(int i) {
            if (i == 0) {
                throw new ArithmeticException("no ratio of zero");
            }
            return i == 7 ? Float.NaN : i == 8 ? -0.0f : (float) limit / i;
        }

        char letter(int i) {
            if (i < 0) {
                throw new IllegalStateException("negative letter");
            }
            return i == 1 ? '￿' : (char) ('a' + i % 26);
        }

        short small(int i) {
            if (i > 40_000) {
                throw new IllegalArgumentException("too large");
            }
            return (short) i;
        }

        byte tiny(int i) {
            if (i == 3) {
                throw new UnsupportedOperationException("three");
            }
            return (byte) (i - 130);
        }
    }

    interface Score {
        int score(int i);
    }

    static final class Strict implements Score {
        public int score(int i) {
            if (i % 5 == 0) {
                throw new IllegalStateException("strict " + i);
            }
            return Integer.MIN_VALUE + i;
        }
    }

    static final class Lenient implements Score {
        private final List<Integer> seen = new ArrayList<>();

        public int score(int i) {
            seen.add(i);
            return -i;
        }
    }

    static String all(Checked c, int i) {
        StringBuilder out = new StringBuilder();
        try {
            out.append(c.index(i)).append(' ');
        } catch (IndexOutOfBoundsException e) {
            out.append("[").append(e.getMessage()).append("] ");
        }
        try {
            out.append(c.odd(i)).append(' ');
        } catch (RuntimeException e) {
            out.append("[").append(e.getMessage()).append("] ");
        }
        try {
            float r = c.ratio(i);
            out.append(r).append(' ').append(Float.floatToRawIntBits(r)).append(' ');
        } catch (ArithmeticException e) {
            out.append("[").append(e.getMessage()).append("] ");
        }
        try {
            out.append((int) c.letter(i)).append(' ');
        } catch (IllegalStateException e) {
            out.append("[").append(e.getMessage()).append("] ");
        }
        try {
            out.append(c.small(i * 1000)).append(' ');
        } catch (IllegalArgumentException e) {
            out.append("[").append(e.getMessage()).append("] ");
        }
        try {
            out.append(c.tiny(i)).append(' ');
        } catch (UnsupportedOperationException e) {
            out.append("[").append(e.getMessage()).append("]");
        }
        return out.toString();
    }

    public static void main(String[] args) {
        Checked c = new Checked(20);
        for (int i : new int[] {-30, -2, 0, 1, 3, 7, 8, 13, 33, 41}) {
            System.out.println(i + ": " + all(c, i));
        }
        Score[] scores = {new Strict(), new Lenient()};
        long total = 0;
        int failed = 0;
        for (int i = 0; i < 12; i++) {
            for (Score s : scores) {
                try {
                    total += s.score(i);
                } catch (IllegalStateException e) {
                    failed++;
                }
            }
        }
        System.out.println(total + " " + failed);
    }
}
