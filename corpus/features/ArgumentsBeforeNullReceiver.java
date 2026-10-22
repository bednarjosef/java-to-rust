// A call on a null object: Java computes the receiver and the arguments, with what they print,
// throw or initialize, before the NullPointerException of the call.
public class ArgumentsBeforeNullReceiver {
    static class Box {
        int v;
        Box next;

        int add(int x) {
            return v + x;
        }

        String name(String s) {
            return s;
        }

        Box find(int x) {
            return next;
        }

        void put(int x, int y) {
            v = x + y;
        }
    }

    static class Holder {
        static final int VALUE = effect("initializing Holder");
    }

    static int effect(String what) {
        System.out.println("evaluated " + what);
        return 1;
    }

    static Box none() {
        System.out.println("evaluated the receiver");
        return null;
    }

    public static void main(String[] args) {
        Box b = args.length > 5 ? new Box() : null;
        try {
            b.add(effect("argument"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        String s = args.length > 5 ? "x" : null;
        try {
            s.indexOf(effect("index"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            s.substring(effect("begin"), effect("end"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        StringBuilder sb = args.length > 5 ? new StringBuilder() : null;
        try {
            sb.append(effect("append"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            none().name("" + effect("name"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        Box outer = new Box();
        try {
            outer.next.put(effect("first"), effect("second"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        // The null that the call returns, once its argument is computed.
        try {
            System.out.println(outer.find(effect("find")).v);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        // The argument throws first.
        int[] small = new int[1];
        try {
            b.add(small[args.length + 3]);
        } catch (NullPointerException | ArrayIndexOutOfBoundsException e) {
            System.out.println("caught " + e);
        }
        // The argument initializes a class.
        try {
            b.add(Holder.VALUE);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        // The argument assigns the receiver's variable, after Java has read it.
        try {
            b.add((b = new Box()).v);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage() + ", b is now " + (b != null));
        }
        // A local that is first a new object, then null.
        Box later = new Box();
        later = none();
        try {
            later.add(effect("later"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        // Quiet arguments: nothing to compute first.
        Box c = args.length > 5 ? new Box() : null;
        int n = 2;
        try {
            c.add(n + 1);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
