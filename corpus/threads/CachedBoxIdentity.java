// `==` on `Boolean` and `Byte` boxes, which are always `valueOf`'s cached objects: the same
// object exactly when the values are equal, also in a static that another thread sets.
public class CachedBoxIdentity {
    static Boolean flag;
    static Byte small;

    public static void main(String[] args) throws Exception {
        Thread t = new Thread(() -> {
            flag = args.length == 0;
            small = (byte) (args.length - 100);
        });
        t.start();
        t.join();
        Boolean yes = Boolean.valueOf(true);
        Boolean no = Boolean.parseBoolean("false");
        Byte same = Byte.valueOf((byte) -100);
        Byte other = (byte) 5;
        System.out.println((flag == yes) + " " + (flag == no) + " " + (flag == Boolean.TRUE) + " " + (flag != null));
        System.out.println((small == same) + " " + (small == other) + " " + (small != null));
        Boolean unset = args.length > 0 ? yes : null;
        System.out.println((unset == null) + " " + (unset == flag));
    }
}
