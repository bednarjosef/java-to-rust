import java.util.Iterator;
import java.util.NoSuchElementException;

public class ForEachEnumIterable {
    enum Side { NORTH, EAST, SOUTH, WEST, UP, DOWN }

    // An enum whose constants are each a group of sides to loop over.
    enum Plane implements Iterable<Side> {
        FLAT(new Side[] { Side.NORTH, Side.EAST, Side.SOUTH, Side.WEST }),
        TALL(new Side[] { Side.UP, Side.DOWN }),
        NONE(new Side[0]);

        private final Side[] sides;

        Plane(Side[] sides) { this.sides = sides; }

        public Iterator<Side> iterator() {
            return new Iterator<Side>() {
                int at;
                public boolean hasNext() { return at < sides.length; }
                public Side next() {
                    if (at >= sides.length) throw new NoSuchElementException();
                    return sides[at++];
                }
            };
        }
    }

    static Plane pick(int i) {
        System.out.println("pick " + i);
        return Plane.values()[i];
    }

    public static void main(String[] args) {
        for (Plane plane : Plane.values()) {
            StringBuilder line = new StringBuilder(plane + ":");
            for (Side side : plane) line.append(' ').append(side);
            System.out.println(line);
        }
        int count = 0;
        outer:
        for (Side side : Plane.FLAT) {
            for (Side other : pick(1)) {
                if (side == Side.SOUTH) break outer;
                if (other == Side.DOWN) continue outer;
                count++;
            }
        }
        System.out.println("count " + count);
        Plane missing = null;
        try {
            for (Side side : missing) System.out.println(side);
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
