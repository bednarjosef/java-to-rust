// A generic method's result used where its bound's type is enough: Java casts the result to the
// type argument only where the place it goes needs that type. javac's `Lower.visitUnary` does
// `result = translate(newTree, tree.type)` with a `JCAssignOp` for `T` and stores into a `JCTree`
// field, and what comes back is not always a `JCAssignOp`.
public class WiderGenericResults {
    interface Named {
        String name();
    }

    abstract static class Tree implements Named {
    }

    static class Expression extends Tree {
        public String name() {
            return "expression";
        }
    }

    static class AssignOp extends Expression {
        public String name() {
            return "assign op";
        }
    }

    static class Binary extends Expression {
        public String name() {
            return "binary";
        }
    }

    static class Translator {
        Tree result;

        // Lowers an assignment operator to a binary expression, as boxing does.
        @SuppressWarnings("unchecked")
        <T extends Expression> T translate(T tree) {
            Expression out = tree instanceof AssignOp ? new Binary() : tree;
            return (T) out;
        }

        void store(AssignOp op) {
            result = translate(op);
        }

        Tree give(AssignOp op) {
            return translate(op);
        }
    }

    static String describe(Tree tree) {
        return tree.name();
    }

    static String describeNamed(Named named) {
        return named.name();
    }

    public static void main(String[] args) {
        Translator t = new Translator();
        t.store(new AssignOp());
        System.out.println("field: " + t.result.name());
        System.out.println("returned: " + t.give(new AssignOp()).name());
        Expression local = t.translate(new AssignOp());
        System.out.println("local: " + local.name());
        Object object = t.translate(new AssignOp());
        System.out.println("object: " + ((Tree) object).name());
        System.out.println("argument: " + describe(t.translate(new AssignOp())));
        System.out.println("interface: " + describeNamed(t.translate(new AssignOp())));
        Tree assigned;
        assigned = t.translate(new AssignOp());
        System.out.println("assigned: " + assigned.name());
        // Where the type argument is needed, the result is checked: this one is a `Binary`, and
        // the program ends with the `ClassCastException`.
        AssignOp op = t.translate(new AssignOp());
        System.out.println("not reached: " + op.name());
    }
}
