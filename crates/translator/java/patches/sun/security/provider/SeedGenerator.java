package sun.security.provider;

/**
 * The translator's patch of the JDK's {@code SeedGenerator} (see docs/jdk-overlays.md). The
 * system's entropy, a digest of what differs between machines and runs, reads the heap's figures
 * from {@code Runtime.getRuntime().totalMemory()} and {@code freeMemory()} called on it directly,
 * as the translation answers them, not through a {@code Runtime} held in a local. A seed source is read only from a file, such as the
 * default {@code file:/dev/random}: a URL of another protocol, which {@code java.security.egd}
 * can name, would open a connection, which the translation does not make, so it throws.
 */
class SeedGenerator {
    @Edit(find = "Runtime rt = Runtime.getRuntime();", replace = "")
    @Edit(find = "longToByteArray(rt.totalMemory())", replace = "longToByteArray(Runtime.getRuntime().totalMemory())")
    @Edit(find = "longToByteArray(rt.freeMemory())", replace = "longToByteArray(Runtime.getRuntime().freeMemory())")
    static byte[] getSystemEntropy();

    @Patch
    static class URLSeedGenerator {
        @Edit(find = "URL device = new URL(deviceName);", replace = """
                URL device = new URL(deviceName);
                if (!device.getProtocol().equalsIgnoreCase("file")) {
                    throw new UnsupportedOperationException(
                            "the seed source " + deviceName + ", not a file, which the translation does not read");
                }
                """)
        // Not reached: the source is a file.
        @Edit(find = "seedStream = device.openStream();", replace = "throw new IllegalStateException(deviceName);")
        private void init();
    }
}
