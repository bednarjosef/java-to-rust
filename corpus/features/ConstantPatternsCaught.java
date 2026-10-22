import java.io.File;

// Splitting and matching with constant patterns that compile, `File.pathSeparator` among them,
// inside a `try` whose `catch` would see a `PatternSyntaxException`: such a pattern cannot be
// malformed, so the calls cannot throw it, as javac's `Locations` splits a module source path.
public class ConstantPatternsCaught {
    static final String DOTS = "\\.";

    public static void main(String[] args) {
        String path = "src/a" + File.pathSeparator + "src/b" + File.pathSeparator + "lib";
        try {
            for (String part : path.split(File.pathSeparator)) {
                System.out.println(part + " " + part.matches("[a-z/]+") + " " + part.split(File.separator).length);
            }
            System.out.println(String.join("|", "a.b.c".split(DOTS)) + " " + "x-y".replaceAll("-", "+"));
            System.out.println(File.separatorChar + " " + File.pathSeparatorChar);
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
