// Objects that leave their method only on some paths: each starts on the stack and moves to the
// heap just before the call or store where it leaves, keeping its fields and its identity.
public class StackObjectsMovedToTheHeap {
    static class Pos {
        int x, y;
        Pos(int x, int y) { this.x = x; this.y = y; }
        int sum() { return x + y; }
        void register() { registered = this; }
    }

    // A subclass makes `Pos` a type of several classes, whose handles carry the class.
    static final class Pos3 extends Pos {
        int z;
        Pos3(int x, int y, int z) { super(x, y); this.z = z; }
        int sum() { return x + y + z; }
    }

    static final class Box {
        Pos held;
        void hold(Pos p) { held = p; }
    }

    static Pos[] kept = new Pos[200];
    static int count;
    static Pos last;
    static Pos registered;
    static int seed = 17;

    static void keep(Pos p) { kept[count++] = p; }

    static Pos random(int base) {
        seed = seed * 1103515245 + 12345;
        return new Pos(base + (seed >>> 16 & 15), seed >>> 8 & 7);
    }

    public static void main(String[] args) {
        int sum = 0;
        Box box = new Box();
        for (int i = 0; i < 60; i++) {
            Pos p = random(i);
            p.x += 1;
            sum += p.sum();
            if (p.y > 5) {
                keep(p);
            }
            if (i % 9 == 0) {
                last = p;
            }
            if (p.x % 11 == 0) {
                box.hold(p);
            }
            if (p.y == 3) {
                p.register();
            }
            // A change after the move is a change of the moved object.
            p.y += 1000;
            for (int j = 0; j < 3; j++) {
                Pos q = new Pos(i, j);
                if (j == 1 && i % 5 == 0) {
                    continue;
                }
                sum += q.sum();
                if (q.x == q.y) {
                    keep(q);
                }
            }
        }
        int big = 0;
        int sameAsLast = 0;
        for (int i = 0; i < count; i++) {
            if (kept[i].y >= 1000) big++;
            if (kept[i] == last) sameAsLast++;
        }
        System.out.println(sum + " " + count + " " + big + " " + sameAsLast);
        System.out.println(last.x + " " + last.y + " " + (box.held == null ? -1 : box.held.y));
        System.out.println(registered == null ? "none" : registered.x + " " + registered.y);

        // An object whose identity hash is read before it leaves keeps the same hash after.
        int stable = 0;
        for (int i = 0; i < 10; i++) {
            Pos p = new Pos(i, i);
            int h = System.identityHashCode(p);
            if (i % 2 == 0) {
                keep(p);
                if (System.identityHashCode(kept[count - 1]) == h) stable++;
            }
        }
        System.out.println(stable);
        Pos other = new Pos3(1, 2, 3);
        keep(other);
        System.out.println(other.sum() + " " + kept[count - 1].sum());
    }
}
