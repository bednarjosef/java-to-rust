// Three levels of classes whose constructors each call overridable methods and pass `this` on:
// each level's body runs on the whole object, after the level above it and before the one below.
// The middle level has objects of its own, its constructors call each other with `this(..)`, and
// an anonymous class extends it.
public class ThreeLevelConstructors {
    static StringBuilder log = new StringBuilder();

    static void note(String s) {
        log.append(s).append('\n');
    }

    abstract static class Base {
        int depth;
        String tag = "base";
        Base origin;

        Base() {
            origin = this;
            depth = 1;
            note("Base sees " + name() + " level " + level());
        }

        abstract String name();

        int level() {
            return depth;
        }
    }

    static class Middle extends Base {
        int count = 2;
        String middleTag;

        {
            note("Middle initializer count " + count + " tag " + tag);
        }

        Middle(String tag) {
            super();
            middleTag = tag;
            depth = depth + 1;
            note("Middle(" + tag + ") sees " + name() + " level " + level());
        }

        Middle() {
            this("default");
            count++;
            note("Middle() count " + count);
        }

        String name() {
            return "middle " + middleTag + " count " + count;
        }

        @Override
        int level() {
            return depth * 10;
        }
    }

    static class Bottom extends Middle {
        long weight = 7;
        Base seen;

        Bottom(long weight) {
            super("bottom");
            note("Bottom(" + weight + ") before, weight " + this.weight + " seen " + (seen == null));
            this.weight = weight;
            seen = origin;
            depth++;
        }

        Bottom() {
            this(99);
            note("Bottom() level " + level());
        }

        @Override
        String name() {
            return "bottom weight " + weight + " via " + super.name();
        }
    }

    public static void main(String[] args) {
        Base a = new Bottom(5);
        note("a: " + a.name() + " level " + a.level() + " origin " + (a.origin == a));
        Base b = new Middle();
        note("b: " + b.name() + " level " + b.level());
        Middle c = new Middle("plain");
        note("c: " + c.name() + " level " + c.level() + " tag " + c.tag);
        Base d = new Bottom();
        note("d: " + d.name() + " seen " + (((Bottom) d).seen == d));
        Base e = new Middle("anonymous") {
            int extra = 1;

            @Override
            String name() {
                return "anonymous extra " + extra + " " + super.name();
            }
        };
        note("e: " + e.name() + " level " + e.level());
        System.out.print(log);
    }
}
