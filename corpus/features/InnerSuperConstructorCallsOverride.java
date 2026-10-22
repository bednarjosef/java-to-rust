// Superclasses with an enclosing instance or captured locals whose constructors call methods
// that subclasses override, or store `this`: the overrides read the enclosing instance and the
// captured locals, which javac sets before `super(..)`, so they are set there.
public class InnerSuperConstructorCallsOverride {
    int total = 5;
    final Base[] made = new Base[8];
    int count;

    // An inner class whose constructor calls an override and stores `this` in its enclosing
    // instance; a subclass in the same class, one that delegates, and an anonymous subclass.
    abstract class Base {
        final String label;
        int extra;

        Base(int extra) {
            this.extra = extra;
            label = describe();
            made[count++] = this;
        }

        Base() {
            this(1);
        }

        abstract String describe();
    }

    class Plain extends Base {
        String describe() {
            return "plain " + (total + extra);
        }
    }

    class Middle extends Base {
        final String tag;

        Middle(String tag) {
            super(10);
            this.tag = tag;
        }

        String describe() {
            return "middle " + total + " " + tag;
        }
    }

    class Leaf extends Middle {
        Leaf() {
            super("leaf");
        }

        String describe() {
            return "leaf " + total * 2;
        }
    }

    Base anonymous(int k) {
        return new Base(k) {
            String describe() {
                return "anonymous " + (total + k);
            }
        };
    }

    // Local classes that capture locals, the superclass's constructor calling the subclass's
    // override, which reads both captures.
    static String locals(String prefix, int scale) {
        StringBuilder out = new StringBuilder();
        abstract class Step {
            final String text;

            Step() {
                text = prefix + render();
                out.append('[').append(text).append(']');
            }

            abstract String render();
        }
        class Doubled extends Step {
            String render() {
                return ":" + scale * 2;
            }
        }
        new Doubled();
        new Step() {
            String render() {
                return "!" + scale + prefix.length();
            }
        };
        return out.toString();
    }

    public static void main(String[] args) {
        InnerSuperConstructorCallsOverride outer = new InnerSuperConstructorCallsOverride();
        System.out.println(outer.new Plain().label);
        System.out.println(outer.new Middle("mid").label);
        System.out.println(outer.new Leaf().label);
        outer.total = 7;
        System.out.println(outer.anonymous(3).label);
        for (int i = 0; i < outer.count; i++) {
            Base b = outer.made[i];
            System.out.println(b.label + " / " + b.extra);
        }
        System.out.println(locals("p", 4));
    }
}
