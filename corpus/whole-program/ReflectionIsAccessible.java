// `isAccessible()`, which the JDK declares in `AccessibleObject`, on reflection's members, asked
// before `setAccessible(true)` as libraries do. (The translation's members are always
// accessible, so the answer itself is not printed: Java's is false until then.)
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

public class ReflectionIsAccessible {
    public int count;

    public ReflectionIsAccessible() {}

    public void callback(int x) {
        count += x;
        System.out.println("called " + x);
    }

    @SuppressWarnings("deprecation")
    public static void main(String[] args) throws Exception {
        Method m = ReflectionIsAccessible.class.getMethod("callback", int.class);
        if (!m.isAccessible()) {
            m.setAccessible(true);
        }
        ReflectionIsAccessible r = new ReflectionIsAccessible();
        m.invoke(r, 3);
        System.out.println(m.isAccessible());
        Field f = ReflectionIsAccessible.class.getField("count");
        if (!f.isAccessible()) {
            f.setAccessible(true);
        }
        System.out.println(f.isAccessible() + " " + f.getInt(r));
        Constructor<ReflectionIsAccessible> c = ReflectionIsAccessible.class.getConstructor();
        c.setAccessible(true);
        System.out.println(c.isAccessible() + " " + c.newInstance().count);
    }
}
