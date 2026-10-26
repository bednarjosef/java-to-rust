import java.util.HashSet;
import java.util.Objects;
import java.util.Set;

// `StackTraceElement`'s `equals` and `hashCode`, as the Minecraft server's
// `CrashReportCategory.validateStackTrace` compares a frame with the one it expects.
public class StackTraceElementEquality {
    private StackTraceElement[] stackTrace;

    StackTraceElementEquality(StackTraceElement... frames) {
        stackTrace = frames;
    }

    boolean validateStackTrace(StackTraceElement source, StackTraceElement next) {
        if (stackTrace.length != 0 && source != null) {
            StackTraceElement current = stackTrace[0];
            if (current.isNativeMethod() == source.isNativeMethod()
                && Objects.equals(current.getClassName(), source.getClassName())
                && Objects.equals(current.getFileName(), source.getFileName())
                && Objects.equals(current.getMethodName(), source.getMethodName())) {
                if (next != null != stackTrace.length > 1) {
                    return false;
                }
                if (next != null && !stackTrace[1].equals(next)) {
                    return false;
                }
                stackTrace[0] = source;
                return true;
            }
        }
        return false;
    }

    public static void main(String[] args) {
        StackTraceElement a = new StackTraceElement("demo.Main", "run", "Main.java", 10);
        StackTraceElement same = new StackTraceElement("demo.Main", "run", "Main.java", 10);
        StackTraceElement otherLine = new StackTraceElement("demo.Main", "run", "Main.java", 11);
        StackTraceElement noFile = new StackTraceElement("demo.Main", "run", null, 10);
        StackTraceElement noFileToo = new StackTraceElement("demo.Main", "run", null, 10);
        StackTraceElement nativeFrame = new StackTraceElement("demo.Main", "poll", null, -2);
        System.out.println(a.equals(same) + " " + a.equals(otherLine) + " " + a.equals(noFile) + " "
            + noFile.equals(noFileToo) + " " + a.equals(null) + " " + a.equals("demo.Main.run(Main.java:10)"));
        System.out.println(a.hashCode() + " " + same.hashCode() + " " + otherLine.hashCode() + " "
            + noFile.hashCode() + " " + nativeFrame.hashCode());
        Object asObject = same;
        System.out.println(asObject.equals(a) + " " + (asObject.hashCode() == a.hashCode()));
        Set<StackTraceElement> frames = new HashSet<>();
        frames.add(a);
        frames.add(same);
        frames.add(otherLine);
        frames.add(noFile);
        frames.add(noFileToo);
        System.out.println(frames.size() + " " + frames.contains(new StackTraceElement("demo.Main", "run", "Main.java", 11)));

        StackTraceElementEquality category = new StackTraceElementEquality(a, otherLine);
        System.out.println(category.validateStackTrace(same, new StackTraceElement("demo.Main", "run", "Main.java", 11)));
        System.out.println(category.validateStackTrace(same, a));
        System.out.println(category.validateStackTrace(same, null));
        System.out.println(new StackTraceElementEquality(a).validateStackTrace(noFile, null));
        System.out.println(new StackTraceElementEquality(noFile).validateStackTrace(noFileToo, null));
    }
}
