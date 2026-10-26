// A `Class[]` held as an `Object` and cast back: an array of the runtime's classes, whose
// runtime class is `Class[]`.
public class ClassArrayAsOneObject {
    public static void main(String[] args) {
        Object o = new Class<?>[] {String.class, Runnable.class};
        Class<?>[] back = (Class<?>[]) o;
        System.out.println(back.length + " " + back[1].getName() + " " + o.getClass().getSimpleName());
    }
}
