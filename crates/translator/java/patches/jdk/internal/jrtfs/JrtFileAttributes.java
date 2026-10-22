package jdk.internal.jrtfs;

/**
 * The translator's patch of the JDK's {@code JrtFileAttributes} (see docs/jdk-overlays.md):
 * {@code toString} throws an {@code UnsupportedOperationException}. The JDK's formats the times
 * with {@code Formatter}'s {@code %tc}, which needs the calendars and time zones that a
 * translated program does not have yet. It is never called by the file system itself.
 */
class JrtFileAttributes {
    @Replace
    @Override
    public final String toString() {
        throw new UnsupportedOperationException("JrtFileAttributes.toString, which formats times with %tc,");
    }
}
