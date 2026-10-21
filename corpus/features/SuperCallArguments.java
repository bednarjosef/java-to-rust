// super.grow(..) from a method of a class with subclasses borrows the fields struct of the
// object; arguments that read the object or call its methods run before that borrow.
public class SuperCallArguments {
    static int calls = 0;
    static class Base {
        int k = 1;
        int grow(int a0) {
            k += a0;
            return k;
        }
    }
    static class Mid extends Base {
        int name() {
            return 1;
        }
        int peek() {
            calls++;
            return k * 10 + name();
        }
        @Override
        int grow(int a0) {
            int r = super.grow(k + a0);
            int s = super.grow(peek());
            return r + s + name();
        }
    }
    static class Left extends Mid {
        @Override
        int name() {
            return 2;
        }
    }
    static class Right extends Mid {
        int right = 3;
    }

    public static void main(String[] args) {
        Mid a = new Left();
        Mid b = new Right();
        Base c = new Base();
        System.out.println(a.grow(1) + " " + b.grow(2) + " " + c.grow(3));
        System.out.println(a.k + " " + b.k + " " + c.k + " " + calls);
    }
}
