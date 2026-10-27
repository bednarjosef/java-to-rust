package java.lang;

import java.io.File;
import java.io.IOException;
import java.util.StringTokenizer;

/**
 * The translator's methods of {@code Runtime}, which is the runtime's class.
 * {@code Runtime.version()} is the version of the JDK that a plain run has, parsed from its
 * {@code java.runtime.version} property, which the translation answers with the value of the JDK
 * it ran with (see {@code lower::system}), as the JVM makes it from the same numbers. The version
 * is made once, so that every caller sees the same one. {@code Runtime.exec} starts a process
 * through a {@code ProcessBuilder}, as the JDK's does.
 */
final class TranslatedRuntime {
    private static Runtime.Version version;

    private TranslatedRuntime() {}

    static synchronized Runtime.Version version() {
        if (version == null) {
            version = Runtime.Version.parse(System.getProperty("java.runtime.version"));
        }
        return version;
    }

    static Process exec(String command) throws IOException {
        return exec(command, null, null);
    }

    static Process exec(String command, String[] envp) throws IOException {
        return exec(command, envp, null);
    }

    /** The command split at white space, as a {@code StringTokenizer} splits it. */
    static Process exec(String command, String[] envp, File dir) throws IOException {
        if (command.isEmpty()) {
            throw new IllegalArgumentException("Empty command");
        }
        StringTokenizer words = new StringTokenizer(command);
        String[] cmdarray = new String[words.countTokens()];
        int count = 0;
        while (words.hasMoreTokens()) {
            cmdarray[count++] = words.nextToken();
        }
        return exec(cmdarray, envp, dir);
    }

    static Process exec(String[] cmdarray) throws IOException {
        return exec(cmdarray, null, null);
    }

    static Process exec(String[] cmdarray, String[] envp) throws IOException {
        return exec(cmdarray, envp, null);
    }

    static Process exec(String[] cmdarray, String[] envp, File dir) throws IOException {
        ProcessBuilder builder = new ProcessBuilder(cmdarray);
        builder.environment(envp);
        builder.directory(dir);
        return builder.start();
    }
}
