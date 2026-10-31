// StackWalker.walk: the stream starts at the frame of the method that walks, which the
// translation knows (its class, its name, the source file and the line of the call). Walks that
// read that frame, or look past it from `main`, where Java has no frame below, see what Java sees.
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

public class StackWalkerFrames {
    private static final StackWalker WALKER = StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE);

    static String describe(StackWalker.StackFrame f) {
        return f.getClassName() + "." + f.getMethodName() + " in " + f.getFileName() + " at "
                + f.getLineNumber() + ", class " + f.getDeclaringClass().getSimpleName()
                + ", native " + f.isNativeMethod();
    }

    static String fromStatic() {
        StackWalker.StackFrame f = WALKER.walk(s -> s.findFirst()).get();
        return describe(f);
    }

    private final String made;

    StackWalkerFrames() {
        made = WALKER.walk(s -> s.map(StackWalker.StackFrame::getMethodName).findFirst().orElse("none"));
    }

    String fromInstance() {
        Optional<StackWalker.StackFrame> f = WALKER.walk(s -> s.findFirst());
        return f.map(StackWalkerFrames::describe).orElse("no frame");
    }

    static class Inner {
        String where() {
            return WALKER.walk(s -> s.findFirst()).map(f -> f.toString()).orElse("none");
        }
    }

    public static void main(String[] args) {
        System.out.println(fromStatic());
        StackWalkerFrames o = new StackWalkerFrames();
        System.out.println(o.made);
        System.out.println(o.fromInstance());
        System.out.println(new Inner().where());
        // Tracy's way: the first frame of another class than the walking one.
        Optional<StackWalker.StackFrame> other = WALKER.walk(s -> s.filter(f -> f.getDeclaringClass() != StackWalkerFrames.class).findFirst());
        System.out.println("below main: " + other.isPresent());
        long count = WALKER.walk(s -> s.count());
        System.out.println("frames in main: " + count);
        List<String> names = WALKER.walk(s -> s.map(f -> f.getMethodName()).collect(Collectors.toList()));
        System.out.println(names);
        StackTraceElement e = WALKER.walk(s -> s.findFirst()).get().toStackTraceElement();
        System.out.println(e.getClassName() + " " + e.getMethodName() + " " + e.getLineNumber());
        try {
            WALKER.walk(null);
        } catch (NullPointerException npe) {
            System.out.println("null function");
        }
    }
}
