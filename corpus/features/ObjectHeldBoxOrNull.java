// A cast of an `Object` that may be `null` to a box fails with HotSpot's message when it holds
// another box or a string, passes a `null` through, and gives the box it holds when it is one.
public class ObjectHeldBoxOrNull {
    public static void main(String[] args) {
        Object o = args.length == 0 ? (Object) 42L : null;
        try {
            int i = (int) o;
            System.out.println(i);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        Object s = args.length == 0 ? (Object) "text" : null;
        try {
            Long l = (Long) s;
            System.out.println(l);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        Object n = args.length == 0 ? null : (Object) 1L;
        Integer k = (Integer) n;
        System.out.println(k);
        Long held = (Long) o;
        System.out.println(held + 1);
        Long none = (Long) n;
        System.out.println(none);
    }
}
