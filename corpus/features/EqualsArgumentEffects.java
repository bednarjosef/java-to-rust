// `Object`'s `equals` on an object that is never an `Object` itself, so it equals nothing:
// its argument, which constructs and calls, is still evaluated.
public class EqualsArgumentEffects {
    static int calls = 0;

    static class Counter {
        int n;
        Counter(int n) {
            this.n = n;
        }
    }
    static class Other {
        int k;
        Other(int k) {
            this.k = k;
            System.out.println("new Other " + k);
        }
    }

    static int bump() {
        calls++;
        System.out.println("bump " + calls);
        return calls;
    }

    public static void main(String[] args) {
        Counter c = new Counter(1);
        c.n = 2;
        System.out.println("equals " + c.equals(new Other(bump())));
        System.out.println("again " + c.equals(new Other(5)) + " " + calls);
        Object o = new Other(6);
        System.out.println("object " + c.equals(o) + " " + c.n);
    }
}
