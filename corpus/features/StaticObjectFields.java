// Static fields of a class or interface type whose initializers make objects of a subclass or
// an implementing class: the initializer converts to the field's type.
public class StaticObjectFields {
    interface Named {
        String name();
    }

    static class Base implements Named {
        int n = 1;
        public String name() { return "base" + n; }
    }

    static class Mid extends Base {
        public String name() { return "mid" + n; }
    }

    static class Leaf extends Mid {
        int m;
        Leaf() { m = 3; }
        public String name() { return "leaf" + m; }
    }

    static Named named = new Leaf();
    static Base base = new Mid();
    static Base other = named instanceof Base b ? b : new Base();

    public static void main(String[] args) {
        System.out.println(named.name() + " " + base.name() + " " + other.name());
        base.n += 5;
        named = base;
        System.out.println(named.name() + " " + (other == base));
    }
}
