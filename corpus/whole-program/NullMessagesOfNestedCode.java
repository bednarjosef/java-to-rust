import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

// The messages of nulls in nested code: a constructor that uses `this` as a value (so its class
// builds whole objects) reading its enclosing instance's field, a class whose superclass has an
// enclosing instance of another class, an element at a constant index, a static of the
// enclosing class, an element at an enum's ordinal, and a for-each variable in a lambda.
public class NullMessagesOfNestedCode {
    Object types = null;
    Handler handler;
    NullMessagesOfNestedCode log = null;
    Object[] stackItem = new Object[5];
    static Object home = null;

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

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    @SuppressWarnings("unchecked")
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
    }
}
