// The messages of a null field of an enclosing instance, which javac reads from the constructor's
// parameter in the constructor's code (`<parameter1>.chk`), also in a lambda there, and from the
// field in the initializers the constructor runs (`this.this$0.chk`), as a captured local
// (`this.val$v`). A lambda in a lambda reads them as the one around it does, its body being a
// method of the same class.
import java.util.Iterator;
import java.util.List;
import java.util.function.Consumer;

public class NullEnclosingInstances {
    Object chk = null;

    class Delegating {
        final String handler;

        Delegating() {
            this(chk.toString());
        }

        Delegating(String handler) {
            this.handler = handler;
        }
    }

    class Assigning {
        final String s;

        Assigning(int k) {
            s = chk.toString() + k;
        }
    }

    class Base {
        Base(String s) {
        }
    }

    class Derived extends Base {
        Derived() {
            super(chk.toString());
        }
    }

    class Initialized {
        String s = chk.toString();
    }

    class Block {
        String s;

        {
            s = chk.toString();
        }
    }

    class Middle {
        class Deep {
            Deep() {
                chk.toString();
            }
        }
    }

    void local() {
        class Local {
            Local(int k) {
                chk.toString();
            }
        }
        new Local(1);
    }

    void anonymous() {
        new Object() {
            String s = chk.toString();
        };
    }

    class InLambda {
        InLambda() {
            Runnable r = () -> chk.toString();
            r.run();
        }
    }

    class InLambdaWithOwn {
        Object own = "x";

        InLambdaWithOwn() {
            Runnable r = () -> {
                own.toString();
                NullEnclosingInstances.this.chk.toString();
            };
            r.run();
        }
    }

    class InitializedLambda {
        Runnable r = () -> chk.toString();

        InitializedLambda() {
            r.run();
        }
    }

    class Lambdas {
        Object lint = null;

        void own(List<String> items) {
            items.forEach(a -> {
                Runnable r = () -> lint.toString();
                r.run();
            });
        }

        void enclosing(List<String> items) {
            items.forEach(a -> {
                Consumer<String> c = b -> chk.toString();
                c.accept(a);
            });
        }

        Lambdas() {
        }

        Lambdas(List<String> items) {
            items.forEach(a -> {
                Runnable r = () -> chk.toString();
                r.run();
            });
        }
    }

    void capturing(List<String> items, String site) {
        items.forEach(a -> {
            Runnable r = () -> {
                if (site.isEmpty()) {
                    return;
                }
                chk.toString();
            };
            r.run();
        });
    }

    void deeper(List<String> items) {
        items.forEach(a -> items.forEach(b -> {
            Runnable r = () -> chk.hashCode();
            r.run();
        }));
    }

    static Iterable<String> wrap(Iterable<String> listed) {
        return () -> new Iterator<String>() {
            private final Iterator<String> original = listed.iterator();

            public boolean hasNext() {
                return original.hasNext();
            }

            public String next() {
                return original.next();
            }
        };
    }

    static void capturedInInitializer(Object v) {
        class Captured {
            String s = v.toString();
        }
        new Captured();
    }

    static void capturedInBlock(Object v) {
        new Object() {
            {
                v.hashCode();
            }
        };
    }

    static void run(Runnable r) {
        try {
            r.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        NullEnclosingInstances o = new NullEnclosingInstances();
        run(() -> o.new Delegating());
        run(() -> o.new Assigning(1));
        run(() -> o.new Derived());
        run(() -> o.new Initialized());
        run(() -> o.new Block());
        run(() -> o.new Middle().new Deep());
        run(() -> o.local());
        run(() -> o.anonymous());
        run(() -> o.new InLambda());
        run(() -> o.new InLambdaWithOwn());
        run(() -> o.new InitializedLambda());
        List<String> one = List.of("x");
        run(() -> o.new Lambdas().own(one));
        run(() -> o.new Lambdas().enclosing(one));
        run(() -> o.new Lambdas(one));
        run(() -> o.capturing(one, "s"));
        run(() -> o.deeper(one));
        run(() -> wrap(null).iterator());
        run(() -> capturedInInitializer(null));
        run(() -> capturedInBlock(null));
    }
}
