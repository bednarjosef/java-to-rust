package jdk.internal.misc;

/**
 * The translator's patch of the JDK's {@code jdk.internal.misc.VM} (see docs/jdk-overlays.md).
 * {@code getRuntimeArguments} asks the JVM for the options it was started with, such as
 * {@code -Xmx} and {@code -D}. A translated program has none, so the answer is the empty array;
 * its one user, {@code CDS}, reads it as an array of any length.
 */
class VM {
    @Replace
    public static String[] getRuntimeArguments() {
        return new String[0];
    }
}
