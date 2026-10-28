import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.util.List;

// `invoke` of a static method's handle whose call site has other reference types than the
// method's, which `invoke` converts (`MethodHandle.asType`): netty's `estimateMaxDirectMemory`
// takes `ManagementFactory.getRuntimeMXBean()` as an `Object` this way. The result is an upcast or
// a checked cast, and so is each argument.
public class StaticHandleInvokeConversions {
    public static void main(String[] args) throws Throwable {
        MethodHandles.Lookup lookup = MethodHandles.lookup();
        MethodHandle get = lookup.findStatic(BeanFactory.class, "bean", MethodType.methodType(Bean.class));
        Object found = (Object) get.invoke();
        List<String> arguments = ((Bean) found).arguments();
        System.out.println(arguments.get(arguments.size() - 1));

        MethodHandle size = lookup.findStatic(BeanFactory.class, "size", MethodType.methodType(int.class, List.class));
        Object items = arguments;
        int n = (int) size.invoke(items);
        System.out.println(n);

        MethodHandle shout = lookup.findStatic(BeanFactory.class, "shout", MethodType.methodType(String.class, String.class));
        Object text = "quiet";
        Object loud = (Object) shout.invoke(text);
        System.out.println(loud);
        CharSequence chars = (CharSequence) shout.invoke(text);
        System.out.println(chars.length());
        try {
            Object number = 7;
            Object never = (Object) shout.invoke(number);
            System.out.println(never);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        MethodHandle any = lookup.findStatic(BeanFactory.class, "any", MethodType.methodType(Object.class));
        try {
            String word = (String) any.invoke();
            System.out.println(word);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
    }
}

interface Bean {
    List<String> arguments();
}

final class BeanFactory {
    public static Bean bean() {
        return () -> List.of("-Xmx1G", "-XX:MaxDirectMemorySize=64m");
    }

    public static int size(List<String> items) {
        return items.size();
    }

    public static String shout(String text) {
        return text.toUpperCase();
    }

    public static Object any() {
        return 7;
    }
}
