// Objects made in the body of a lambda whose interface is a closure type, and that never leave
// the lambda's method, are made on the stack of each call of the closure: a fresh one per call,
// also when the closure calls itself again before the first call ends, and one kept by a call
// is moved to the heap first.
public class ObjectsMadeInLambdaBodies {
    static final class Vec {
        final double x, y, z;

        Vec(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        Vec add(Vec o) {
            return new Vec(x + o.x, y + o.y, z + o.z);
        }

        double length() {
            return Math.sqrt(x * x + y * y + z * z);
        }
    }

    static final class Pos {
        int x, y, z;

        Pos(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }
    }

    interface StepVisitor {
        boolean visit(Pos p, int step);
    }

    interface Walker {
        int walk(int depth);
    }

    static final Vec[] kept = new Vec[8];
    static int keptCount;

    static int forEachStep(int from, int to, StepVisitor visitor) {
        int visited = 0;
        for (int i = from; i < to; i++) {
            if (!visitor.visit(new Pos(i, i * 2, i * 3), i - from)) {
                break;
            }
            visited++;
        }
        return visited;
    }

    static double shapeSize(Vec offset, Vec size) {
        return offset.add(size).length();
    }

    static Walker walker;

    public static void main(String[] args) {
        double[] total = {0};
        Vec size = new Vec(0.5, 1, 0.5);
        int steps = forEachStep(0, 40, (p, step) -> {
            // Stays: read by `shapeSize` and dropped.
            Vec offset = new Vec(p.x, p.y, p.z);
            total[0] += shapeSize(offset, size);
            // Leaves on some steps only: moved to the heap there.
            Vec corner = new Vec(p.x + 1, p.y + 1, p.z + 1);
            if (step % 9 == 0) {
                kept[keptCount++] = corner;
            }
            return step < 30;
        });
        System.out.println(steps + " " + Math.round(total[0] * 1000) + " " + keptCount);
        double sum = 0;
        for (int i = 0; i < keptCount; i++) {
            sum += kept[i].x + kept[i].y + kept[i].z;
        }
        System.out.println(sum);

        // A closure that calls itself: each call's object is its own.
        walker = depth -> {
            Vec here = new Vec(depth, depth + 1, depth + 2);
            int below = depth > 0 ? walker.walk(depth - 1) : 0;
            return below + (int) (here.x + here.y + here.z);
        };
        System.out.println(walker.walk(6));
    }
}
