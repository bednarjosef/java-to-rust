// A call on an object that a pattern variable reaches: Java computes the arguments, with what
// they print, before the NullPointerException of a null receiver.
public class PatternReceiverArguments {
    static class Box {
        Box next;
        int v;

        int add(int x) {
            return v + x;
        }

        boolean has(Tag t) {
            return t == Tag.A;
        }
    }

    enum Tag {
        A,
        B
    }

    // javac's `Check.typeTagError`: the pattern variable is the receiver, an enum constant the
    // argument, which initializes its class; then a pattern of the same name in an argument.
    static String tagError(Object found) {
        if (found instanceof Box box && box.has(Tag.A)) {
            return "has";
        }
        return String.valueOf(found).concat(found instanceof Box box ? " a box of " + box.v : " other");
    }

    static int effect(String what) {
        System.out.println("evaluated " + what);
        return 1;
    }

    public static void main(String[] args) {
        Object o = new Box();
        System.out.println(tagError(o));
        System.out.println(tagError("text"));
        if (o instanceof Box b && b.add(effect("chained")) > 0) {
            System.out.println("chained");
        }
        if (o instanceof Box b) {
            try {
                b.next.add(effect("then"));
            } catch (NullPointerException e) {
                System.out.println("caught " + e.getMessage());
            }
        }
        Object p = args.length > 3 ? new Box() : "text";
        System.out.println(o instanceof Box d ? d.add(effect("conditional")) : 0);
        System.out.println(p instanceof Box d && d.next.add(effect("not run")) > 0);
        if (!(o instanceof Box c)) {
            return;
        }
        try {
            c.next.add(effect("after"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
