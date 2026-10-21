// shared.toString() on an interface value is Object's toString, which borrows the object as
// turning it into text does, so it conflicts with shared.bump() in the same statement.
public class InterfaceToStringBorrow {
    interface Counter {
        int bump();
    }
    static class Tally implements Counter {
        int n;
        String label;
        Tally(String label) {
            this.label = label;
        }
        public int bump() {
            n++;
            return n;
        }
        @Override
        public String toString() {
            return "Tally(" + label + ", " + n + ")";
        }
    }
    static Counter shared = new Tally("hello");

    public static void main(String[] args) {
        System.out.println("a " + shared.bump() + " " + shared.toString());
        System.out.println("b " + shared + " " + shared.bump() + " " + shared);
        Counter local = shared;
        System.out.println("c " + local.bump() + " " + local.toString() + " " + (shared == local));
    }
}
