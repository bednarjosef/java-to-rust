public class QualifiedInnerCreation {
    int k = 3;
    String name = "box";

    class Inner {
        int v() { return k + name.length(); }
    }

    class Sub extends Inner {
        Sub() { super(); }
    }

    public static void main(String[] a) {
        QualifiedInnerCreation o = new QualifiedInnerCreation();
        try {
            System.out.println(o.new Inner().v());
            System.out.println(o.new Sub().v());
            QualifiedInnerCreation n = a.length > 5 ? o : null;
            System.out.println(n.new Inner().v());
        } catch (NullPointerException e) {
            System.out.println("npe " + e.getMessage());
        }
        OuterHolder holder = new OuterHolder();
        try {
            System.out.println(holder.outer().new Inner() {
                int v() { return 1 + super.v(); }
            }.v());
            holder.empty = true;
            System.out.println(holder.outer().new Inner() {
                int v() { return 2; }
            }.v());
        } catch (NullPointerException e) {
            System.out.println("anonymous npe " + e.getMessage());
        }
    }

    static class OuterHolder {
        boolean empty;

        QualifiedInnerCreation outer() {
            return empty ? null : new QualifiedInnerCreation();
        }
    }
}
