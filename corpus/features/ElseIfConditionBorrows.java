// The condition of an `else if` that reads a field of an object and calls a method that writes
// that object back, as javac's `Check.checkNonCyclicInternal` does with `else if
// (!c.type.isErroneous())`, which completes the symbol `c`. The condition's borrows are its own
// statement's: the field is read out before the call, as in a first `if`.
public class ElseIfConditionBorrows {
    abstract static class Node {
        Node other;
        int hits;

        abstract String kind();

        boolean visit() {
            other.hits++;
            return other.hits % 2 == 1;
        }
    }

    static class Leaf extends Node {
        String kind() {
            return "leaf";
        }
    }

    static class Branch extends Node {
        String kind() {
            return "branch";
        }
    }

    static String classify(boolean first, Node n) {
        if (first) {
            return "first";
        } else if (n.other.visit()) {
            return "odd " + n.kind() + " " + n.hits;
        } else {
            return "even " + n.kind() + " " + n.hits;
        }
    }

    public static void main(String[] args) {
        Node a = new Leaf();
        Node b = new Branch();
        a.other = b;
        b.other = a;
        System.out.println(classify(true, a));
        for (int i = 0; i < 3; i++) {
            System.out.println(classify(false, a));
            System.out.println(classify(false, b));
        }
    }
}
