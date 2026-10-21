// Particles on springs, each pair of neighbours pulling at each other, written the plain way: the
// array element particles[j] is read again for every field it uses, rather than once into a
// local. Each such read of an element that is an object is a load in Java; a translation that
// copies the element's handle out of the array at every read pays for it many times per step.
public class ParticleArray {
    static class Particle {
        double x, y, vx, vy, mass;

        Particle(double x, double y, double mass) {
            this.x = x;
            this.y = y;
            this.mass = mass;
        }
    }

    private final Particle[] particles;

    ParticleArray(int n) {
        particles = new Particle[n];
        for (int i = 0; i < n; i++) {
            particles[i] = new Particle(i, (i * 7) % 5, 1.0 + (i % 3));
        }
    }

    void step(double dt) {
        for (int i = 0; i + 1 < particles.length; i++) {
            int j = i + 1;
            double dx = particles[j].x - particles[i].x;
            double dy = particles[j].y - particles[i].y;
            double distance = Math.sqrt(dx * dx + dy * dy);
            double force = (distance - 1.0) * 0.5;
            double fx = force * dx / distance;
            double fy = force * dy / distance;
            particles[i].vx += fx / particles[i].mass * dt;
            particles[i].vy += fy / particles[i].mass * dt;
            particles[j].vx -= fx / particles[j].mass * dt;
            particles[j].vy -= fy / particles[j].mass * dt;
        }
        for (int i = 0; i < particles.length; i++) {
            particles[i].x += particles[i].vx * dt;
            particles[i].y += particles[i].vy * dt;
        }
    }

    double energy() {
        double e = 0;
        for (int i = 0; i < particles.length; i++) {
            e += 0.5 * particles[i].mass * (particles[i].vx * particles[i].vx + particles[i].vy * particles[i].vy);
        }
        return e;
    }

    public static void main(String[] args) {
        ParticleArray system = new ParticleArray(100);
        for (int t = 0; t < 2000000; t++) {
            system.step(0.001);
        }
        System.out.printf("%.9f%n", system.energy());
    }
}
