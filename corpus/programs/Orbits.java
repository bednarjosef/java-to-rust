/**
 * A small n-body simulation: bodies orbit a heavy star under gravity, integrated with the
 * leapfrog method. It keeps a growing log of energies (System.arraycopy), bins the bodies'
 * angles around the star (Math.atan2, Math.floorMod), and fingerprints the final state from the
 * bits of its doubles (Double.doubleToLongBits), so that a change in the last bit shows.
 */
public class Orbits {
    static double[] grow(double[] a, int size) {
        if (size < a.length) {
            return a;
        }
        double[] bigger = new double[a.length * 2];
        System.arraycopy(a, 0, bigger, 0, a.length);
        return bigger;
    }

    static double energy(double[] x, double[] y, double[] vx, double[] vy, double[] m) {
        double kinetic = 0.0;
        double potential = 0.0;
        for (int i = 0; i < m.length; i++) {
            kinetic += 0.5 * m[i] * (vx[i] * vx[i] + vy[i] * vy[i]);
            for (int j = i + 1; j < m.length; j++) {
                potential -= m[i] * m[j] / Math.hypot(x[i] - x[j], y[i] - y[j]);
            }
        }
        return kinetic + potential;
    }

    static void accelerate(double[] x, double[] y, double[] ax, double[] ay, double[] m) {
        for (int i = 0; i < m.length; i++) {
            ax[i] = 0.0;
            ay[i] = 0.0;
        }
        for (int i = 0; i < m.length; i++) {
            for (int j = 0; j < m.length; j++) {
                if (i != j) {
                    double dx = x[j] - x[i];
                    double dy = y[j] - y[i];
                    double r2 = dx * dx + dy * dy + 1e-9;
                    double inv = m[j] / (r2 * Math.sqrt(r2));
                    ax[i] += dx * inv;
                    ay[i] += dy * inv;
                }
            }
        }
    }

    static long fingerprint(double[] values, long seed) {
        long h = seed;
        for (int i = 0; i < values.length; i++) {
            long bits = Double.doubleToLongBits(values[i]);
            h = (h ^ bits) * 0x100000001b3L;
            h ^= h >>> 29;
        }
        return h;
    }

    public static void main(String[] args) {
        int n = 6;
        double[] m = new double[n];
        double[] x = new double[n];
        double[] y = new double[n];
        double[] vx = new double[n];
        double[] vy = new double[n];
        m[0] = 1000.0;
        for (int i = 1; i < n; i++) {
            double r = 2.0 + 1.5 * i;
            double phase = StrictMath.toRadians(73.0 * i);
            m[i] = 0.01 * i;
            x[i] = r * StrictMath.cos(phase);
            y[i] = r * StrictMath.sin(phase);
            double v = Math.sqrt(m[0] / r);
            vx[i] = -v * StrictMath.sin(phase);
            vy[i] = v * StrictMath.cos(phase);
        }
        double[] ax = new double[n];
        double[] ay = new double[n];
        accelerate(x, y, ax, ay, m);

        double dt = 0.001;
        double[] log = new double[4];
        int logged = 0;
        double e0 = energy(x, y, vx, vy, m);
        for (int step = 1; step <= 4000; step++) {
            for (int i = 0; i < n; i++) {
                vx[i] += 0.5 * dt * ax[i];
                vy[i] += 0.5 * dt * ay[i];
                x[i] += dt * vx[i];
                y[i] += dt * vy[i];
            }
            accelerate(x, y, ax, ay, m);
            for (int i = 0; i < n; i++) {
                vx[i] += 0.5 * dt * ax[i];
                vy[i] += 0.5 * dt * ay[i];
            }
            if (step % 100 == 0) {
                log = grow(log, logged + 1);
                log[logged] = energy(x, y, vx, vy, m);
                logged++;
            }
            if (step % 500 == 0) {
                System.out.println("step " + step);
                for (int i = 1; i < n; i++) {
                    double angle = StrictMath.toDegrees(Math.atan2(y[i] - y[0], x[i] - x[0]));
                    int sector = Math.floorMod((int) Math.floor(angle / 45.0), 8);
                    double distance = Math.hypot(x[i] - x[0], y[i] - y[0]);
                    System.out.println("  body " + i + " sector " + sector + " distance " + distance
                        + " angle " + Math.round(angle * 1000.0) / 1000.0);
                }
            }
        }

        double worst = 0.0;
        for (int k = 0; k < logged; k++) {
            double drift = Math.abs((log[k] - e0) / e0);
            worst = Math.max(worst, drift);
        }
        System.out.println("energy " + e0 + " logged " + logged + " capacity " + log.length);
        System.out.println("worst relative drift " + worst + " log10 " + StrictMath.log10(worst));
        double[] state = new double[4 * n];
        System.arraycopy(x, 0, state, 0, n);
        System.arraycopy(y, 0, state, n, n);
        System.arraycopy(vx, 0, state, 2 * n, n);
        System.arraycopy(vy, 0, state, 3 * n, n);
        System.out.println("fingerprint " + fingerprint(state, 0xcbf29ce484222325L));
        System.out.println("orbital period of body 1 " + 2 * Math.PI * Math.sqrt(StrictMath.pow(3.5, 3) / m[0]));
    }
}
