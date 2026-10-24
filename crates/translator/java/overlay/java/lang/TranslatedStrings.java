package java.lang;

import java.util.Arrays;
import java.util.Comparator;
import java.util.Spliterator;
import java.util.function.IntConsumer;
import java.util.stream.IntStream;
import java.util.stream.StreamSupport;

/**
 * The static fields of {@code String} that are objects, which the translator reads here because
 * its strings are the runtime's and {@code String} is not translated (see
 * {@code program::redirected_field}), and the streams of a string's code points.
 */
final class TranslatedStrings {
    /** {@code String.CASE_INSENSITIVE_ORDER}: one comparator, as the JDK has. */
    static final Comparator<String> CASE_INSENSITIVE_ORDER = new CaseInsensitiveComparator();

    private TranslatedStrings() {}

    /**
     * Compares as {@code compareToIgnoreCase} does, which the JDK writes as this comparison. The
     * JDK's first reads the text of each string, so a null one throws as reading its field does.
     */
    private static final class CaseInsensitiveComparator implements Comparator<String>, java.io.Serializable {
        public int compare(String s1, String s2) {
            if (s1 == null) {
                throw new NullPointerException("Cannot read field \"value\" because \"s1\" is null");
            }
            if (s2 == null) {
                throw new NullPointerException("Cannot read field \"value\" because \"s2\" is null");
            }
            return s1.compareToIgnoreCase(s2);
        }
    }

    /**
     * {@code s.codePoints()}, given the string's UTF-16 code units (see
     * {@code program::reach::STRING_STREAMS}). The JDK keeps a string whose characters are all
     * Latin-1 as bytes and streams them as {@code chars()} does, a {@code SIZED} stream; any other
     * string streams through a spliterator that is not {@code SIZED} and estimates its size as the
     * number of units left, which {@code CodePoints} is.
     */
    static IntStream codePoints(int[] units) {
        for (int unit : units) {
            if (unit > 0xFF) {
                return StreamSupport.intStream(new CodePoints(units, 0, units.length), false);
            }
        }
        return Arrays.stream(units);
    }

    /**
     * The code points of the units from {@code index} to {@code fence}: a high surrogate followed by
     * a low one is one code point, any other unit its own. A split never parts a pair.
     */
    private static final class CodePoints implements Spliterator.OfInt {
        private final int[] units;
        private int index;
        private final int fence;

        CodePoints(int[] units, int index, int fence) {
            this.units = units;
            this.index = index;
            this.fence = fence;
        }

        @Override
        public OfInt trySplit() {
            int start = index, middle = (start + fence) >>> 1;
            if (start >= middle) {
                return null;
            }
            // A middle between the two halves of a pair moves back to before the pair, unless the
            // pair is all there is.
            if (Character.isLowSurrogate((char) units[middle])
                    && Character.isHighSurrogate((char) units[middle - 1])) {
                if (start >= middle - 1) {
                    return null;
                }
                middle--;
            }
            index = middle;
            return new CodePoints(units, start, middle);
        }

        @Override
        public void forEachRemaining(IntConsumer action) {
            if (action == null) {
                throw new NullPointerException();
            }
            int i = index, end = fence;
            index = end;
            while (i < end) {
                i = next(i, end, action);
            }
        }

        @Override
        public boolean tryAdvance(IntConsumer action) {
            if (action == null) {
                throw new NullPointerException();
            }
            if (index >= fence) {
                return false;
            }
            index = next(index, fence, action);
            return true;
        }

        /** Gives the code point at {@code i} to the action and returns where the next one starts. */
        private int next(int i, int end, IntConsumer action) {
            char high = (char) units[i++];
            int point = high;
            if (Character.isHighSurrogate(high) && i < end && Character.isLowSurrogate((char) units[i])) {
                point = Character.toCodePoint(high, (char) units[i++]);
            }
            action.accept(point);
            return i;
        }

        @Override
        public long estimateSize() {
            return fence - index;
        }

        @Override
        public int characteristics() {
            return Spliterator.ORDERED | Spliterator.IMMUTABLE;
        }
    }
}
