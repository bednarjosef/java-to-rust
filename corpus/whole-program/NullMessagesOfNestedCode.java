import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

// The messages of nulls in nested code: a constructor that uses `this` as a value (so its class
// builds whole objects) reading its enclosing instance's field, a class whose superclass has an
// enclosing instance of another class, an element at a constant index, a static of the
// enclosing class, an element at an enum's ordinal, an enum's `toString()`, a for-each variable
// in a lambda, code under a constant `false`, which javac writes no code for, and the
// `subSequence` of a `CharSequence`'s text, which is lowered as a `substring`, a null
// `CharSequence` that `contentEquals` compares, and a null that a call returns in a lambda
// whose argument dereferences, as in javac's `Check.checkIfTypeParamsRequiresIdentity`.
public class NullMessagesOfNestedCode {
    Object types = null;
    Handler handler;
    NullMessagesOfNestedCode log = null;
    Object[] stackItem = new Object[5];
    static Object home = null;
    private final boolean debug = false;
    Phase phase;

    interface Codes {
        int OBJECTcode = 4;
    }

    enum Phase { BASIC, BOX }

    abstract class Context {
        boolean strict() {
            return types == null;
        }
    }

    class Check {
        class CheckContext extends Context {
            boolean compatible(Object found) {
                return types.equals(found);
            }
        }
    }

    abstract class Handler {
        Handler prev;

        Handler() {
            prev = log.handler;
            handler = this;
        }
    }

    class Deferred extends Handler {
    }

    class Member {
        int duplicate() {
            return stackItem[Codes.OBJECTcode].hashCode();
        }
    }

    class Location {
        int path() {
            return home.hashCode();
        }
    }

    static int phase(List<String>[] itypes, Phase p) {
        int n = 0;
        for (String s : itypes[p.ordinal()]) {
            n += s.length();
        }
        return n;
    }

    static class Sym {
        Object owner;
    }

    static void owners(List<? extends Collection<Sym>> groups, Object site) {
        groups.forEach(list -> {
            for (Sym m : list) {
                if (m.owner != site) {
                    continue;
                }
            }
        });
    }

    int traced(Sym s) {
        if (debug) {
            System.out.println(s.owner.hashCode());
        }
        return s.owner.hashCode();
    }

    String phaseName() {
        return phase.toString() + phase.name();
    }

    static class Name implements CharSequence {
        public int length() {
            return 0;
        }

        public char charAt(int index) {
            return 'n';
        }

        public CharSequence subSequence(int start, int end) {
            return toString().subSequence(start, end);
        }

        @Override
        public String toString() {
            return null;
        }
    }

    static Name flatName() {
        return null;
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    @SuppressWarnings("unchecked")
    static class Seq<A> extends java.util.AbstractList<A> {
        final List<A> items = new ArrayList<>();

        public A get(int i) {
            return items.get(i);
        }

        public int size() {
            return items.size();
        }
    }

    static class Position {
        final int index;

        Position(int index) {
            this.index = index;
        }
    }

    static class Tree {
        final String type;

        Tree(String type) {
            this.type = type;
        }
    }

    static List<String> typed(List<Position> at, Seq<Tree> trees) {
        List<String> out = new ArrayList<>();
        at.stream()
            .filter(p -> trees.get(p.index).type != null && trees.get(p.index).type.isEmpty())
            .forEach(p -> out.add("at " + p.index));
        return out;
    }

    public static void main(String[] args) {
        NullMessagesOfNestedCode o = new NullMessagesOfNestedCode();
        run(() -> o.new Check().new CheckContext().compatible("x"));
        run(() -> o.new Deferred());
        run(() -> o.new Member().duplicate());
        run(() -> o.new Location().path());
        run(() -> phase(new List[2], Phase.BOX));
        List<Sym> list = new ArrayList<>();
        list.add(null);
        run(() -> owners(List.of(list), "s"));
        run(() -> o.traced(new Sym()));
        run(() -> System.out.println(o.phaseName()));
        run(() -> System.out.println(new Name().subSequence(0, 1)));
        run(() -> System.out.println("n".contentEquals(flatName())));
        Seq<Tree> trees = new Seq<>();
        trees.items.add(new Tree(""));
        trees.items.add(new Tree(null));
        trees.items.add(null);
        run(() -> System.out.println(typed(List.of(new Position(0), new Position(1)), trees)));
        run(() -> System.out.println(typed(List.of(new Position(0), new Position(2)), trees)));
    }
}
