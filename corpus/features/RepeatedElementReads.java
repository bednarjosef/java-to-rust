// An array element read several times in a row of statements that cannot change it is read
// once, at its first read: the later reads see the same object, and an index out of bounds or a
// null element still fails where Java's first read fails, after what came before it.
public class RepeatedElementReads {
    static class Body {
        double x, v, mass;

        Body(double x, double mass) {
            this.x = x;
            this.mass = mass;
        }
    }

    static String[] names = {"ab", "cde", null};
    private final Body[] bodies;
    private int calls;

    RepeatedElementReads(int n) {
        bodies = new Body[n];
        for (int i = 0; i < n; i++) {
            bodies[i] = new Body(i * 1.5, 1.0 + i);
        }
    }

    void step(double dt) {
        for (int i = 0; i < bodies.length; i++) {
            Body a = bodies[i];
            for (int j = i + 1; j < bodies.length; j++) {
                double dx = a.x - bodies[j].x;
                double d = Math.sqrt(dx * dx + 0.01);
                a.v -= dx * bodies[j].mass / d * dt;
                bodies[j].v += dx * a.mass / d * dt;
            }
        }
        for (Body b : bodies) {
            b.x += b.v * dt;
        }
    }

    double sum(int k) {
        calls++;
        double s = (calls += 10) + bodies[k].x;
        s += bodies[k].mass;
        return s;
    }

    static int replaced(Body[] p, Body q) {
        double before = p[0].mass;
        p[0] = q;
        double after = p[0].mass;
        return (int) (before * 10 + after);
    }

    static double conditional(Body[] p, int i, boolean look) {
        boolean heavy = look && p[i].mass > 2;
        double m = p[i].mass + p[i].x;
        return heavy ? m : -m;
    }

    static int moving(Body[] p) {
        int i = 0;
        double first = p[i].mass + p[i++].mass + p[i].mass;
        return (int) first + i;
    }

    public static void main(String[] args) {
        RepeatedElementReads system = new RepeatedElementReads(4);
        for (int t = 0; t < 100; t++) {
            system.step(0.01);
        }
        for (Body b : system.bodies) {
            System.out.printf("%.6f %.6f%n", b.x, b.v);
        }
        System.out.println(system.sum(2));
        try {
            System.out.println(system.sum(7));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage() + " after " + system.calls);
        }
        Body[] p = {new Body(0, 3), new Body(1, 4)};
        System.out.println(replaced(p, new Body(2, 7)));
        System.out.println(conditional(p, 1, true) + " " + conditional(p, 1, false));
        System.out.println(moving(p));
        String twice = names[1] + names[1] + names[0];
        System.out.println(twice);
        try {
            String s = names[2] + "!";
            System.out.println(s + names[2]);
            Body[] holes = new Body[2];
            holes[0] = p[0];
            double m = holes[0].mass + holes[1].mass + holes[1].x;
            System.out.println(m);
        } catch (NullPointerException e) {
            System.out.println("null element");
        }
    }
}
