// A field that may be null, declared by a superclass and written through objects of its
// subclasses, through variables, array elements and itself.
public class InheritedNullableFields {
    static class Base {
        int n = 1;
        Base link;
        String name() { return "base" + n; }
    }

    static class Mid extends Base {
        String name() { return "mid" + n; }
    }

    static class Leaf extends Mid {
        int m = 4;
        String name() { return "leaf" + m; }
    }

    public static void main(String[] args) {
        Leaf a = new Leaf();
        Mid b = new Mid();
        a.link = b;
        b.link = a;
        System.out.println(a.link.name() + " " + b.link.name());
        a.link = a;
        Base[] arr = {a, b};
        arr[1].link = a;
        System.out.println(a.link.name() + " " + (arr[1].link == a) + " " + (a.link.link == a));
        b.link = null;
        System.out.println(b.link == null);
        arr[0].link = null;
        System.out.println(a.link == null ? "cut" : a.link.name());
    }
}
