// A primitive field read again within a run of statements is read once only where nothing in the
// run can change it: a store to a field of the same name through another reference to the same
// object, a call, or a volatile field of that name must each see the new value.
public class FieldReadsInRuns {
    static class Body {
        double x, vx, mass;
        int hits;

        Body(double x, double mass) {
            this.x = x;
            this.mass = mass;
        }
    }

    static class Flag {
        volatile int count;
    }

    static class Counter {
        int count;
    }

    static double pull(Body a, Body b) {
        double dx = a.x - b.x;
        a.vx -= dx * b.mass;
        b.vx += dx * a.mass;
        a.vx -= dx * b.mass * 0.5;
        return a.vx + b.vx + a.mass * b.mass;
    }

    static double aliased(Body a, Body b) {
        double before = a.mass;
        b.mass = before * 2;
        return a.mass + before;
    }

    static int bump(Body b) {
        b.hits++;
        return b.hits;
    }

    static int withCall(Body a) {
        int first = a.hits;
        int seen = bump(a);
        return first + a.hits + seen;
    }

    static int counted(Flag f, Counter c) {
        int a = f.count;
        int b = c.count;
        c.count = b + 1;
        return a + f.count + c.count;
    }

    public static void main(String[] args) {
        Body a = new Body(1.5, 2.0);
        Body b = new Body(-0.5, 3.0);
        System.out.println(pull(a, b));
        System.out.println(pull(a, b));
        System.out.println(aliased(a, a));
        System.out.println(aliased(a, b));
        System.out.println(withCall(a));
        Flag f = new Flag();
        f.count = 4;
        Counter c = new Counter();
        System.out.println(counted(f, c));
        System.out.println(a.x + " " + a.vx + " " + a.mass + " " + b.vx + " " + b.mass);
    }
}
