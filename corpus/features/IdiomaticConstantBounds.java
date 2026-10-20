public class IdiomaticConstantBounds {
    static final int LIMIT = 10;
    static final int TOP = Integer.MAX_VALUE;
    static final int FLOOR = -5;
    static final long LONG_TOP = Long.MAX_VALUE;
    static final int WRAP_STEPS = 5;

    public static void main(String[] args) {
        // A counter bounded by a constant: the increment cannot overflow, and the sum stays small.
        int total = 0;
        int i = 0;
        while (i < LIMIT) {
            total = total + i;
            i = i + 1;
        }
        System.out.println(total + " " + i);

        // The counter reaches the largest int exactly: `i < TOP` makes `i + 1` fit.
        int near = Integer.MAX_VALUE - 3;
        int nearSteps = 0;
        while (near < TOP) {
            near = near + 1;
            nearSteps++;
        }
        System.out.println(near + " " + nearSteps);

        // A counter that climbs down to a negative constant bound.
        int down = 0;
        while (down > FLOOR) {
            down = down - 1;
        }
        System.out.println(down);

        // `k <= TOP` never ends on its own: `k + 1` wraps to the smallest int, and the loop goes on.
        int k = Integer.MAX_VALUE - 2;
        int steps = 0;
        while (k <= TOP) {
            k = k + 1;
            steps++;
            if (steps > WRAP_STEPS) break;
        }
        System.out.println(k + " " + steps);

        // The same with a long constant at its largest value: the increment wraps.
        long big = Long.MAX_VALUE - 1;
        int bigSteps = 0;
        while (bigSteps < 3) {
            big = big + 1;
            bigSteps++;
        }
        System.out.println(big + " " + bigSteps + " " + (LONG_TOP == big));
    }
}
