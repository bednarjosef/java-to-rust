package jdk.internal.jrtfs;

/**
 * The translator's patch of the JDK's {@code SystemImage} (see docs/jdk-overlays.md), the class
 * library behind the {@code jrt:/} file system. The JDK's finds the JDK's home from where its own
 * class was loaded (a {@code CodeSource}, through {@code AccessController}), which for the boot
 * loader is the {@code java.home} property; patched, it reads that property, which a translated
 * program answers when it runs (`lower/system.rs`). A JDK without {@code lib/modules} (its
 * exploded modules, as it is built) is refused with an {@code UnsupportedOperationException}.
 */
@Edit(from = "PrivilegedAction<String> pa = SystemImage::findHome;", to = "RUNTIME_HOME = AccessController.doPrivileged(pa);",
        replace = "RUNTIME_HOME = System.getProperty(\"java.home\");")
@Edit(from = "modulesImageExists = AccessController.doPrivileged(", to = "});",
        replace = "modulesImageExists = Files.isRegularFile(moduleImageFile);")
class SystemImage {
    // The modules of a JDK as it is built, a directory of each module's classes, are not read:
    // the JDK's reader of them is not translated.
    @Edit(find = "return new ExplodedImage(explodedModulesDir);",
            replace = "throw new UnsupportedOperationException(\"the exploded modules of \" + explodedModulesDir);")
    static SystemImage open();

    @Remove
    private static String findHome();
}
