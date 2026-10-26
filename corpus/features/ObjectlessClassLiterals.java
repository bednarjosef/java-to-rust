public class ObjectlessClassLiterals {
    public static void main(String[] args) {
        Class<?> runtime = Runtime.class;
        Class<?> system = System.class;
        System.out.println(runtime.getName() + " " + runtime.getSimpleName());
        System.out.println(system.getName() + " " + system.getSimpleName());
        System.out.println(runtime == Runtime.class);
        System.out.println(runtime.equals(system));
        System.out.println(Object.class.isAssignableFrom(runtime));
        System.out.println(runtime);
    }
}
