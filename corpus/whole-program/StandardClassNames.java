import java.lang.reflect.Array;
import java.util.ArrayList;
import java.util.List;

// The names of the standard classes the translator provides, as reflection and a failed cast give
// them: `java.lang.Thread`, and `java.lang.Thread$UncaughtExceptionHandler` for a nested one.
public class StandardClassNames {
    static Object[] frames(Class<?> type, int length) {
        return (Object[]) Array.newInstance(type, length);
    }

    public static void main(String[] args) {
        Object text = "abc";
        try {
            Thread thread = (Thread) text;
            System.out.println(thread.getName());
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        List<StackTraceElement> trace = new ArrayList<>();
        Object[] copied = trace.toArray(new StackTraceElement[0]);
        System.out.println(copied.getClass().getName());
        Object[] made = frames(StackTraceElement.class, 2);
        System.out.println(made.getClass().getName() + " " + made.length);
        System.out.println(Thread.UncaughtExceptionHandler.class.getName());
        System.out.println(Thread.UncaughtExceptionHandler.class.getSimpleName());
    }
}
