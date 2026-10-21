// A constructor whose assignments run in another order than its class declares the fields, as
// Are We Fast Yet's DeltaBlue `Strength` does: the first assignment keeps the parameter and the
// second passes it to a method that keeps it too. The constructor builds its object as a struct
// literal, whose values run in the order of the assignments.
public class ConstructorAssignmentOrder {
    static class Sym {
        final String name;

        Sym(String name) {
            this.name = name;
        }
    }

    static class Table {
        static Sym last;

        static int code(Sym s) {
            last = s;
            return s.name.length();
        }
    }

    static class Strength {
        private final int value;
        private final Sym sym;

        Strength(Sym sym) {
            this.sym = sym;
            this.value = Table.code(sym);
        }
    }

    public static void main(String[] args) {
        Strength s = new Strength(new Sym("required"));
        System.out.println(s.value + " " + s.sym.name + " " + (Table.last == s.sym));
    }
}
