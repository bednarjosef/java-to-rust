// Enum fields, array elements and return values that may be null, as Are We Fast Yet's Havlak
// (`new BasicBlockClass[size]`, whose elements start null) and DeltaBlue (a `Direction` field
// set to null, and `chooseMethod` returning it) write them: each is an `Option` of the enum, set,
// compared, tested against null, printed, returned and dropped.
public class NullEnumPlaces {
    enum Kind { TOP, NONHEADER, REDUCIBLE, SELF, IRREDUCIBLE, DEAD }

    enum Direction { FORWARD, BACKWARD }

    abstract static class Constraint {
        abstract Direction choose(int mark);
    }

    static class Binary extends Constraint {
        Direction direction;
        int marked;

        Binary(int marked) {
            this.marked = marked;
            direction = null;
        }

        @Override
        Direction choose(int mark) {
            if (mark == marked) {
                direction = null;
                return direction;
            }
            direction = mark < marked ? Direction.FORWARD : Direction.BACKWARD;
            return direction;
        }

        boolean satisfied() {
            return direction != null;
        }

        String output() {
            return direction == Direction.FORWARD ? "v2" : "v1";
        }
    }

    static class Unary extends Constraint {
        @Override
        Direction choose(int mark) {
            return null;
        }
    }

    static Kind[] kinds = new Kind[4];

    public static void main(String[] args) {
        Kind[] type = new Kind[6];
        System.out.println(type[0] + " " + (type[0] == null) + " " + (type[1] == Kind.DEAD));
        for (int w = 0; w < type.length; w++) {
            type[w] = w % 2 == 0 ? Kind.NONHEADER : Kind.DEAD;
        }
        type[3] = Kind.SELF;
        type[4] = null;
        int selfs = 0;
        for (int w = 0; w < type.length; w++) {
            if (type[w] == Kind.SELF || type[w] != Kind.NONHEADER && type[w] != null) {
                selfs++;
            }
            System.out.print(type[w] + " ");
        }
        System.out.println(selfs);
        kinds[2] = Kind.TOP;
        kinds[1] = kinds[2];
        System.out.println(kinds[0] + " " + kinds[1] + " " + (kinds[1] == kinds[2]) + " " + (kinds[0] != kinds[3]));

        Binary b = new Binary(5);
        Constraint[] constraints = {b, new Unary()};
        for (Constraint c : constraints) {
            c.choose(3);
        }
        System.out.println(b.satisfied() + " " + b.direction + " " + b.output());
        System.out.println(b.choose(5) + " " + b.satisfied() + " " + b.output());
        System.out.println(b.choose(7) + " " + (b.choose(9) == Direction.BACKWARD) + " " + b.output());
        System.out.println(new Unary().choose(1) == null);
    }
}
