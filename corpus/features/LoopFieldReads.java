// Fields that a loop only reads, taken once before the loop: what the loop reads must still be
// what Java reads, when another name writes the same object, when the loop writes a field of that
// name of another object, when a call or a class's initialization could write it, when the
// object may be null, and in loops that never turn.
public class LoopFieldReads {
    static class Body {
        double x, y, mass;
        double vx, vy;
        int hits;

        Body(double x, double y, double mass) {
            this.x = x;
            this.y = y;
            this.mass = mass;
        }

        void nudge() {
            x += 0.5;
        }

        // `this.mass` is read at every turn, and nothing in the loop writes a `mass`.
        double weighed(double[] values) {
            double total = 0;
            for (int i = 0; i < values.length; i++) {
                total += values[i] * mass;
            }
            return total;
        }
    }

    static class Late {
        static double scale = 2.0;

        static {
            System.out.println("Late initialized");
        }
    }

    static class Counter {
        int limit;
        int count;

        Counter(int limit) {
            this.limit = limit;
        }
    }

    // The n-body step: bi.x, bi.y and bi.mass stay the same through the inner loop; vx and vy change.
    static void step(Body[] bodies) {
        for (int i = 0; i < bodies.length; i++) {
            Body bi = bodies[i];
            for (int j = i + 1; j < bodies.length; j++) {
                Body bj = bodies[j];
                double dx = bi.x - bj.x;
                double dy = bi.y - bj.y;
                double d2 = dx * dx + dy * dy + 0.01;
                double mag = 0.01 / (d2 * Math.sqrt(d2));
                bi.vx -= dx * bj.mass * mag;
                bi.vy -= dy * bj.mass * mag;
                bj.vx += dx * bi.mass * mag;
                bj.vy += dy * bi.mass * mag;
            }
        }
        for (Body b : bodies) {
            b.x += 0.01 * b.vx;
            b.y += 0.01 * b.vy;
        }
    }

    // The loop writes `x` through another name of the same object: every read sees the write.
    static double aliased(Body a, Body b, int n) {
        double sum = 0;
        for (int i = 0; i < n; i++) {
            sum += a.x;
            b.x = b.x + 1;
        }
        return sum;
    }

    // A call in the loop may write the field.
    static double called(Body a, int n) {
        double sum = 0;
        for (int i = 0; i < n; i++) {
            sum += a.x;
            a.nudge();
        }
        return sum;
    }

    // A class initialized in the loop runs code (it prints) before the read.
    static double initialized(Body a, int n) {
        double sum = 0;
        for (int i = 0; i < n; i++) {
            sum += a.x * Late.scale;
        }
        return sum;
    }

    // The object may be null: the read throws at the turn where Java reads it, after the prints
    // before it, and not at all in a loop that never turns.
    static double maybeNull(Body a, int n) {
        double sum = 0;
        for (int i = 0; i < n; i++) {
            System.out.println("turn " + i);
            sum += a.mass;
        }
        return sum;
    }

    // While and do-while loops, with the limit a field of an object the loop counts with.
    static int whileLoops(Counter c, Counter other) {
        int turns = 0;
        while (c.count < c.limit) {
            c.count++;
            turns += other.limit;
        }
        do {
            turns += c.limit;
            other.count += 2;
        } while (other.count < other.limit);
        return turns;
    }

    // The variable is assigned in the loop: each turn reads the field of the object it names then.
    static double reassigned(Body[] bodies) {
        double sum = 0;
        Body b = bodies[0];
        for (int i = 0; i < bodies.length; i++) {
            sum += b.mass;
            b = bodies[(i + 1) % bodies.length];
        }
        return sum;
    }

    // A loop that never turns reads nothing, and an inner loop with a label left early.
    static int labeled(Counter[] counters, int n) {
        int found = 0;
        Counter first = counters[0];
        outer:
        for (int i = 0; i < counters.length; i++) {
            for (int k = 0; k < n; k++) {
                if (k * first.limit > 20) {
                    continue outer;
                }
                found += first.limit;
            }
        }
        return found;
    }

    public static void main(String[] args) {
        Body[] bodies = {
            new Body(0, 0, 10), new Body(1, 0, 1), new Body(0, 2, 0.5), new Body(-3, 1, 0.25),
        };
        for (int s = 0; s < 1000; s++) {
            step(bodies);
        }
        for (Body b : bodies) {
            System.out.println(b.x + " " + b.y + " " + b.vx + " " + b.vy);
        }
        Body a = new Body(1, 2, 3);
        System.out.println(aliased(a, a, 5));
        System.out.println(aliased(a, new Body(0, 0, 0), 5));
        System.out.println(called(a, 4));
        System.out.println(initialized(a, 3));
        System.out.println(maybeNull(a, 2));
        System.out.println(maybeNull(null, 0));
        try {
            maybeNull(null, 3);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getClass().getName());
        }
        System.out.println(whileLoops(new Counter(5), new Counter(7)));
        System.out.println(reassigned(bodies));
        System.out.println(labeled(new Counter[] {new Counter(3), new Counter(9)}, 10));
        System.out.println(bodies[1].weighed(new double[] {1.5, 2.5, 3.5}));
    }
}
