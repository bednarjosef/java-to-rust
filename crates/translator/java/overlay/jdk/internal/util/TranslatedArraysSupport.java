package jdk.internal.util;

/**
 * The translator's methods that calls of some of {@code ArraysSupport}'s call instead
 * ({@code program::REDIRECTED_CALLS}).
 */
public final class TranslatedArraysSupport {
    private TranslatedArraysSupport() {}

    /**
     * {@code ArraysSupport.vectorizedMismatch}, which compares the arrays a {@code long} at a time
     * at offsets from their base addresses, and gives the first mismatch it finds or the
     * complement of how many elements are left to compare. This one compares none and leaves all
     * {@code length} of them: each caller compares what is left one element at a time, as Java
     * defines the result.
     */
    public static int vectorizedMismatch(Object a, long aOffset, Object b, long bOffset, int length,
                                         int log2ArrayIndexScale) {
        return ~length;
    }
}
