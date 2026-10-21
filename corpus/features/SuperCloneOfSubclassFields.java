// `super.clone()` in a superclass's `clone`, run for an object of a subclass: the copy has the
// subclass's fields too, which the superclass's code does not name, and the superclass's `clone`
// then changes its own fields of the copy.
public class SuperCloneOfSubclassFields {
    abstract static class Shelf implements Cloneable {
        int[] cache = {7};
        String label = "shelf";

        @Override
        public Shelf clone() {
            try {
                Shelf copy = (Shelf) super.clone();
                copy.cache = null;
                return copy;
            } catch (CloneNotSupportedException e) {
                throw new AssertionError(e);
            }
        }

        abstract String describe();
    }

    static class Rack extends Shelf {
        final String kind;
        int slots;
        long[] weights;

        Rack(String kind, int slots) {
            this.kind = kind;
            this.slots = slots;
            this.weights = new long[] {slots, 2L * slots};
        }

        @Override
        String describe() {
            return label + " " + kind + " " + slots + " " + weights[1] + " " + (cache == null);
        }
    }

    public static void main(String[] args) {
        Rack rack = new Rack("steel", 4);
        Shelf copy = rack.clone();
        rack.slots = 5;
        rack.weights[1] = 99;
        System.out.println(rack.describe());
        System.out.println(copy.describe());
        System.out.println(copy instanceof Rack && copy != rack);
    }
}
