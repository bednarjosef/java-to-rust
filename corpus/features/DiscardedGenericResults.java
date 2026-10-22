// A generic method whose result erasure would check is called as a statement, as javac's
// `Lower.visitNewClass` calls `translate(tree.def)`: `translate` gives back the block that
// replaces an anonymous class's declaration, and Java casts a generic method's result to its
// type argument only where the value is used.
public class DiscardedGenericResults {
    abstract static class Tree {
        abstract String name();
    }

    static class ClassDef extends Tree {
        String name() {
            return "class";
        }
    }

    static class Block extends Tree {
        String name() {
            return "block";
        }
    }

    static class Translator {
        Tree result;

        @SuppressWarnings("unchecked")
        <T extends Tree> T translate(T tree) {
            result = tree instanceof ClassDef ? new Block() : tree;
            return (T) result;
        }
    }

    public static void main(String[] args) {
        Translator t = new Translator();
        t.translate(new ClassDef());
        System.out.println("discarded: " + t.result.name());
        Tree kept = t.translate(new Block());
        System.out.println("kept: " + kept.name());
        // A used result is checked: this one is a `Block`, and the program ends with the
        // `ClassCastException`.
        ClassDef def = t.translate(new ClassDef());
        System.out.println("not reached: " + def.name());
    }
}
