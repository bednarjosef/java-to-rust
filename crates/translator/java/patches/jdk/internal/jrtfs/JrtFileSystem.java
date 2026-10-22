package jdk.internal.jrtfs;

/**
 * The translator's patch of the JDK's {@code JrtFileSystem} (see docs/jdk-overlays.md): a
 * {@code RuntimeException} of the image's reader is thrown on, not given as the reason of an
 * {@code InvalidPathException} ({@code lookup}).
 */
class JrtFileSystem {
    // The JDK's also gives a `RuntimeException` of the image's reader, which reads a valid image
    // without one, as the reason of an `InvalidPathException`: here it is thrown on, as a
    // translated program has the messages of few of the `NullPointerException`s that the
    // reader's code could raise.
    @Edit(find = "catch (RuntimeException | IOException ex)", replace = "catch (IOException ex)")
    private Node lookup(String path);
}
