// The messages of a null field of an enclosing instance, which javac reads from the constructor's
// parameter in the constructor's code (`<parameter1>.chk`), also in a lambda there, and from the
// field in the initializers the constructor runs (`this.this$0.chk`).
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
    }
}
