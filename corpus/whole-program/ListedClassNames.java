import java.util.ArrayList;
import java.util.List;

// Classes found by names that one string lists, as a provider's `SupportedKeyClasses` attribute
// lists the key interfaces (`"java.security.interfaces.ECPublicKey" +
// "|java.security.interfaces.ECPrivateKey"`) that `Provider.Service` splits and finds with
// `Class.forName`.
public class ListedClassNames {
    static List<Class<?>> classes(String listed) {
        List<Class<?>> found = new ArrayList<>();
        for (String name : listed.split("\\|")) {
            try {
                found.add(Class.forName(name));
            } catch (ClassNotFoundException e) {
                System.out.println("not found: " + name);
            }
        }
        return found;
    }

    static boolean supports(List<Class<?>> classes, Object value) {
        for (Class<?> c : classes) {
            if (c.isAssignableFrom(value.getClass())) {
                return true;
            }
        }
        return false;
    }

    public static void main(String[] args) {
        String keyClasses = "java.util.RandomAccess" +
                "|java.lang.Runnable";
        List<Class<?>> found = classes(keyClasses);
        System.out.println(found.size());
        System.out.println(supports(found, new ArrayList<String>()));
        Runnable task = () -> {};
        System.out.println(supports(found, task));
        System.out.println(supports(found, 42));
        System.out.println(classes("java.util.RandomAccess,java.lang.Runnable".replace(',', '|')).size());
    }
}
