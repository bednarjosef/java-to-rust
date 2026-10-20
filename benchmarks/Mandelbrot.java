// Escape-time counts of the Mandelbrot set over a square grid: a double loop around a
// data-dependent inner loop of floating-point arithmetic.
public class Mandelbrot {
    static int escapeSteps(double cr, double ci, int maxIter) {
        double zr = 0;
        double zi = 0;
        int i = 0;
        while (i < maxIter && zr * zr + zi * zi <= 4.0) {
            double t = zr * zr - zi * zi + cr;
            zi = 2 * zr * zi + ci;
            zr = t;
            i++;
        }
        return i;
    }

    static long render(int size, int maxIter) {
        long total = 0;
        for (int py = 0; py < size; py++) {
            double ci = 2.0 * py / size - 1.0;
            for (int px = 0; px < size; px++) {
                double cr = 2.0 * px / size - 1.5;
                total += escapeSteps(cr, ci, maxIter);
            }
        }
        return total;
    }

    public static void main(String[] args) {
        int size = 1200;
        int maxIter = 1000;
        for (int warm = 0; warm < 2; warm++) {
            render(size, maxIter);
        }
        long start = System.nanoTime();
        long total = render(size, maxIter);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("grid " + size + " by " + size + " iterations " + total);
        System.err.println("kernel-ms " + elapsed);
    }
}
