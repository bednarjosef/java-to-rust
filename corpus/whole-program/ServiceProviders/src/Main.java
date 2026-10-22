import java.util.Iterator;
import java.util.ServiceLoader;
import shapes.Shape;

// `ServiceLoader` over the providers that `META-INF/services` declares: found in the file's order,
// made lazily by their constructors and kept by the loader's iterators (which `findFirst` asks),
// made again by a provider's `get()`, and none for a service without providers. Then the class
// loaders: the system one, its parent the platform one, and its classes found by name.
public class Main {
    public static void main(String[] args) throws Exception {
        ServiceLoader<Shape> loader = ServiceLoader.load(Shape.class);
        System.out.println(loader);
        for (Shape s : loader) {
            System.out.println(s.name() + " " + s.area());
        }
        Iterator<Shape> again = loader.iterator();
        System.out.println(again.next().name());
        System.out.println(loader.stream().map(p -> p.type().getName()).toList());
        System.out.println(loader.findFirst().map(Shape::name).orElse("none"));
        System.out.println(loader.stream().findFirst().get().get().area());
        loader.reload();
        System.out.println(loader.iterator().next().name());
        ServiceLoader<Runnable> none = ServiceLoader.load(Runnable.class);
        System.out.println(none.iterator().hasNext() + " " + none.findFirst().isPresent());

        ClassLoader system = ClassLoader.getSystemClassLoader();
        System.out.println(system.getName() + " " + system.getParent().getName() + " "
                + (system.getParent().getParent() == null));
        System.out.println((Shape.class.getClassLoader() == system) + " " + (String.class.getClassLoader() == null));
        Class<?> circle = system.loadClass("shapes.Circle");
        System.out.println(circle.getName() + " " + Shape.class.isAssignableFrom(circle));
        try {
            system.loadClass("shapes.Triangle");
        } catch (ClassNotFoundException e) {
            System.out.println("no " + e.getMessage().startsWith("shapes.Triangle"));
        }
    }
}
