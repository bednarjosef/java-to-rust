package com.sun.tools.javac.main;

/**
 * The translator's patch of javac's {@code Main} (see docs/jdk-overlays.md). The hidden option
 * {@code -XDshowClass} prints where javac's own class file is and its SHA-256 checksum; a
 * translated javac has no class files, and the checksum would need the JDK's security providers,
 * so the option fails loudly, saying so.
 */
class Main {
    @Replace
    void showClass(String className) {
        throw new UnsupportedOperationException(
                "-XDshowClass, which prints the checksum of javac's own class file, is not supported by the translation");
    }
}
