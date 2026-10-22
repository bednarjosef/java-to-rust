// A visitor whose interface's default method passes `this` on (`d.accept(this, null)`, as
// `ModuleElement.DirectiveVisitor.visit` does), implemented by one class whose only field is
// final (`PrintingProcessor`'s `PrintDirective`): its objects escape through that method.
public class DefaultMethodPassesThis {
    interface Directive {
        <R, P> R accept(Visitor<R, P> v, P p);
    }

    interface Visitor<R, P> {
        default R visit(Directive d) {
            return d.accept(this, null);
        }

        R visitRequires(Requires r, P p);

        R visitExports(Exports e, P p);
    }

    record Requires(String module) implements Directive {
        public <R, P> R accept(Visitor<R, P> v, P p) {
            return v.visitRequires(this, p);
        }
    }

    static final class Exports implements Directive {
        private final String pkg;
        private final String[] to;

        Exports(String pkg, String... to) {
            this.pkg = pkg;
            this.to = to;
        }

        String pkg() {
            return pkg;
        }

        String[] to() {
            return to;
        }

        public <R, P> R accept(Visitor<R, P> v, P p) {
            return v.visitExports(this, p);
        }
    }

    static final class Printer implements Visitor<Integer, String> {
        private final StringBuilder out;

        Printer(StringBuilder out) {
            this.out = out;
        }

        public Integer visitRequires(Requires r, String p) {
            out.append("requires ").append(r.module()).append(";\n");
            return out.length();
        }

        public Integer visitExports(Exports e, String p) {
            out.append("exports ").append(e.pkg());
            if (e.to().length > 0) {
                out.append(" to ").append(String.join(", ", e.to()));
            }
            out.append(";\n");
            return out.length();
        }
    }

    public static void main(String[] args) {
        StringBuilder out = new StringBuilder();
        Printer printer = new Printer(out);
        Directive[] directives = {new Requires("java.base"), new Exports("a.b"), new Exports("c", "x", "y")};
        for (Directive d : directives) {
            System.out.println(printer.visit(d));
        }
        System.out.print(out);
    }
}
