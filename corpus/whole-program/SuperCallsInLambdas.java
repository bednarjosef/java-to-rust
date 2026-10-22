// A `super` call in a lambda runs the overridden method on the object that the lambda captured,
// as javac's ThisEscapeAnalyzer does with `visitScoped(true, () -> super.visitLetExpr(tree))`. The
// code that cannot run is left out: `Printer.visit`, which uses what the translation does not
// support, never runs, since no `Printer` is visited, and is no reason to reject the program.
public class SuperCallsInLambdas {
    static class Scanner {
        void visit(int depth) {
            System.out.println("scanner at " + depth);
        }
    }

    static class Analyzer extends Scanner {
        int scopes = 0;

        void scoped(Runnable body) {
            scopes++;
            body.run();
        }

        @Override
        void visit(int depth) {
            scoped(() -> super.visit(depth));
        }
    }

    static class Printer extends Scanner {
        @Override
        void visit(int depth) {
            System.out.println(Printer.class.getProtectionDomain());
        }
    }

    public static void main(String[] args) {
        Analyzer analyzer = new Analyzer();
        Scanner scanner = analyzer;
        scanner.visit(1);
        scanner.visit(2);
        System.out.println("scopes: " + analyzer.scopes + ", " + new Printer().getClass().getSimpleName());
    }
}
