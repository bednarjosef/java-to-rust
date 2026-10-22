// `getClass()` of an object that may be null, or of a field of one, where a `catch` sees the
// `NullPointerException`: the object is read, and a null throws.
public class ClassOfNullCaught {
    interface Proc { int run(); }

    static class Plain implements Proc {
        public int run() { return 1; }
    }

    static class State {
        Proc processor;
        State(Proc processor) { this.processor = processor; }
    }

    static State find(int k) { return k > 0 ? new State(new Plain()) : null; }

    static Plain plain(int k) { return k > 0 ? new Plain() : null; }

    public static void main(String[] args) {
        System.out.println(plain(1).run());
        try {
            State ps = find(1);
            System.out.println(ps.processor.getClass().getName());
            ps = find(0);
            System.out.println(ps.processor.getClass().getName());
        } catch (NullPointerException e) {
            System.out.println("caught the field's object");
        }
        try {
            State ps = find(0);
            System.out.println(ps.processor.getClass());
        } catch (NullPointerException e) {
            System.out.println("caught the class's object");
        }
        try {
            Plain p = plain(1);
            System.out.println(p.getClass().getSimpleName());
            p = plain(0);
            System.out.println(p.getClass().getName());
        } catch (NullPointerException e) {
            System.out.println("caught: " + e.getMessage());
        }
        try {
            Plain a = plain(1);
            Plain b = plain(0);
            System.out.println(a.getClass() == b.getClass());
        } catch (NullPointerException e) {
            System.out.println("caught the compared one");
        }
    }
}
