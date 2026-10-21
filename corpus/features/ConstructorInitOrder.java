// `new C(args)` initializes C (and its superclass) before it evaluates the arguments, whose
// static reads and calls see what the static initializers did.
public class ConstructorInitOrder {
    static int sInt = 32;
    static String log = "";

    static class Base {
        static int count = (sInt++);
        int k;
        Base(int k) {
            this.k = k;
        }
    }
    static class Leaf extends Base {
        static int made = sInt * 2;
        Leaf(int k) {
            super(k + 1);
            made++;
        }
    }
    static class Other {
        static int seen = next();
        int v;
        Other(int v) {
            this.v = v;
        }
    }

    static int next() {
        log += "next ";
        return sInt += 10;
    }

    static Base first = new Leaf(sInt);

    public static void main(String[] args) {
        System.out.println(first.k + " " + Base.count + " " + Leaf.made + " " + sInt);
        Other o = new Other(sInt);
        System.out.println(o.v + " " + Other.seen + " " + log);
        Other p = new Other(sInt);
        System.out.println(p.v + " " + Other.seen);
    }
}
