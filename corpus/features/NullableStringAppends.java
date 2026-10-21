// `+=` on `String` fields and a static that are null until then, which append to `null`:
// own and inherited fields, through a variable of the superclass, with a `char` and an `int`.
public class NullableStringAppends {
    static class A {
        String name;
        int k;
        A(int k) {
            this.k = k;
        }
    }
    static class B extends A {
        String tag;
        B(int k) {
            super(k);
        }
    }
    static String sText;
    public static void main(String[] args) {
        A a = new A(1);
        a.name += "x";
        System.out.println(a.name);
        a.name += 5;
        System.out.println(a.name + " " + a.k);
        B b = new B(2);
        b.name += "y";
        b.tag += "z";
        b.tag += b.name;
        System.out.println(b.name + b.tag);
        A c = new B(3);
        c.name += 'q';
        System.out.println(c.name);
        sText += "s";
        System.out.println(sText);
        String local = null;
        local += "t";
        System.out.println(local);
    }
}
