// Values whose type is the intersection `Record & Spline` that javac infers for a generic call over
// two records of one interface, and a lambda that casts its result to `Record`: they are held as
// `Record`s, and the records' own `equals`, `hashCode` and `toString` answer through it.
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

public class RecordIntersections {
    interface Spline {
        float at(float x);
    }

    record Constant(float value) implements Spline {
        public float at(float x) {
            return value;
        }
    }

    record Line(float slope, float offset) implements Spline {
        public float at(float x) {
            return slope * x + offset;
        }
    }

    record Either<L, R>(L left, R right) {
        static <L, R> Either<L, R> ofLeft(L value) {
            return new Either<>(value, null);
        }

        static <L, R> Either<L, R> ofRight(R value) {
            return new Either<>(null, value);
        }

        <T> T map(Function<? super L, ? extends T> l, Function<? super R, ? extends T> r) {
            return left != null ? l.apply(left) : r.apply(right);
        }
    }

    static Spline spline(Either<Float, Line> e) {
        return (Spline) e.map(Constant::new, line -> line);
    }

    static Spline merge(Either<Constant, Either<Line, Constant>> e) {
        return (Spline) e.map(Function.identity(), inner -> (Record) inner.map(Function.identity(), c -> c));
    }

    public static void main(String[] args) {
        List<Spline> splines = new ArrayList<>();
        splines.add(spline(Either.ofLeft(2.5f)));
        splines.add(spline(Either.ofRight(new Line(2, 1))));
        splines.add(merge(Either.ofLeft(new Constant(4))));
        splines.add(merge(Either.ofRight(Either.ofLeft(new Line(-1, 3)))));
        splines.add(merge(Either.ofRight(Either.ofRight(new Constant(7)))));
        for (Spline s : splines) {
            System.out.println(s + " " + s.at(2) + " " + s.hashCode());
        }
        List<Record> records = new ArrayList<>();
        for (Spline s : splines) {
            records.add((Record) s);
        }
        records.add(Either.ofLeft("x"));
        System.out.println(records.contains(new Constant(7)) + " " + records.indexOf(new Line(-1, 3)));
        System.out.println(records.get(5) + " " + records.get(5).equals(Either.ofLeft("x")));
    }
}
