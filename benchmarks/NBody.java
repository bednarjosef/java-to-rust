// The n-body simulation of the Sun and four planets: floating-point arithmetic on objects held in
// an array, with a square root per pair of bodies in every step.
public class NBody {
    static final double PI = 3.141592653589793;
    static final double SOLAR_MASS = 4 * PI * PI;
    static final double DAYS_PER_YEAR = 365.24;

    static class Body {
        double x, y, z, vx, vy, vz, mass;

        Body(double x, double y, double z, double vx, double vy, double vz, double mass) {
            this.x = x;
            this.y = y;
            this.z = z;
            this.vx = vx;
            this.vy = vy;
            this.vz = vz;
            this.mass = mass;
        }
    }

    static Body[] system() {
        Body sun = new Body(0, 0, 0, 0, 0, 0, SOLAR_MASS);
        Body jupiter = new Body(4.84143144246472090e+00, -1.16032004402742839e+00, -1.03622044471123109e-01,
                1.66007664274403694e-03 * DAYS_PER_YEAR, 7.69901118419740425e-03 * DAYS_PER_YEAR,
                -6.90460016972063023e-05 * DAYS_PER_YEAR, 9.54791938424326609e-04 * SOLAR_MASS);
        Body saturn = new Body(8.34336671824457987e+00, 4.12479856412430479e+00, -4.03523417114321381e-01,
                -2.76742510726862411e-03 * DAYS_PER_YEAR, 4.99852801234917238e-03 * DAYS_PER_YEAR,
                2.30417297573763929e-05 * DAYS_PER_YEAR, 2.85885980666130812e-04 * SOLAR_MASS);
        Body uranus = new Body(1.28943695621391310e+01, -1.51111514016986312e+01, -2.23307578892655734e-01,
                2.96460137564761618e-03 * DAYS_PER_YEAR, 2.37847173959480950e-03 * DAYS_PER_YEAR,
                -2.96589568540237556e-05 * DAYS_PER_YEAR, 4.36624404335156298e-05 * SOLAR_MASS);
        Body neptune = new Body(1.53796971148509165e+01, -2.59193146099879641e+01, 1.79258772950371181e-01,
                2.68067772490389322e-03 * DAYS_PER_YEAR, 1.62824170038242295e-03 * DAYS_PER_YEAR,
                -9.51592254519715870e-05 * DAYS_PER_YEAR, 5.15138902046611451e-05 * SOLAR_MASS);
        Body[] bodies = {sun, jupiter, saturn, uranus, neptune};
        double px = 0, py = 0, pz = 0;
        for (Body b : bodies) {
            px += b.vx * b.mass;
            py += b.vy * b.mass;
            pz += b.vz * b.mass;
        }
        sun.vx = -px / SOLAR_MASS;
        sun.vy = -py / SOLAR_MASS;
        sun.vz = -pz / SOLAR_MASS;
        return bodies;
    }

    static void advance(Body[] bodies, double dt) {
        for (int i = 0; i < bodies.length; i++) {
            Body bi = bodies[i];
            for (int j = i + 1; j < bodies.length; j++) {
                Body bj = bodies[j];
                double dx = bi.x - bj.x;
                double dy = bi.y - bj.y;
                double dz = bi.z - bj.z;
                double d2 = dx * dx + dy * dy + dz * dz;
                double mag = dt / (d2 * Math.sqrt(d2));
                bi.vx -= dx * bj.mass * mag;
                bi.vy -= dy * bj.mass * mag;
                bi.vz -= dz * bj.mass * mag;
                bj.vx += dx * bi.mass * mag;
                bj.vy += dy * bi.mass * mag;
                bj.vz += dz * bi.mass * mag;
            }
        }
        for (Body b : bodies) {
            b.x += dt * b.vx;
            b.y += dt * b.vy;
            b.z += dt * b.vz;
        }
    }

    static double energy(Body[] bodies) {
        double e = 0;
        for (int i = 0; i < bodies.length; i++) {
            Body bi = bodies[i];
            e += 0.5 * bi.mass * (bi.vx * bi.vx + bi.vy * bi.vy + bi.vz * bi.vz);
            for (int j = i + 1; j < bodies.length; j++) {
                Body bj = bodies[j];
                double dx = bi.x - bj.x;
                double dy = bi.y - bj.y;
                double dz = bi.z - bj.z;
                e -= bi.mass * bj.mass / Math.sqrt(dx * dx + dy * dy + dz * dz);
            }
        }
        return e;
    }

    static double run(int steps) {
        Body[] bodies = system();
        for (int i = 0; i < steps; i++) {
            advance(bodies, 0.01);
        }
        return energy(bodies);
    }

    public static void main(String[] args) {
        int steps = 15000000;
        for (int warm = 0; warm < 2; warm++) {
            run(steps);
        }
        long start = System.nanoTime();
        double before = energy(system());
        double after = run(steps);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("energy before " + before);
        System.out.println("energy after " + after);
        System.err.println("kernel-ms " + elapsed);
    }
}
