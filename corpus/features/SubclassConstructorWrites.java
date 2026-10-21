// Constructors of subclasses whose first statements assign fields of a superclass, alone and
// mixed with fields of their own: the superclass's fields are built by `super(...)` first.
public class SubclassConstructorWrites {
    static class Base {
        int n = 1;
        String tag = "base";
    }

    static class Mid extends Base {
        int m;
        Mid() {
            n = 3;
            m = n + 1;
        }
        Mid(int k) {
            m = k;
            n = k * 2;
            tag = "mid";
        }
    }

    static class Leaf extends Mid {
        Leaf() {
            super(5);
            tag = tag + "/leaf";
        }
    }

    public static void main(String[] args) {
        Mid a = new Mid();
        Mid b = new Mid(4);
        Leaf c = new Leaf();
        System.out.println(a.n + " " + a.m + " " + a.tag);
        System.out.println(b.n + " " + b.m + " " + b.tag);
        System.out.println(c.n + " " + c.m + " " + c.tag);
    }
}
