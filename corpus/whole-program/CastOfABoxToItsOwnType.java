import java.util.function.Function;

// A cast of a box to its own type, which the decompiler writes where a generic method returns
// the box (`(Float) transformer.apply(value)`, as Minecraft's `CubicSpline.Builder` does), then
// unboxed: a null throws when it is unboxed, with the message of the call.
public class CastOfABoxToItsOwnType {
    interface FloatFunction extends Function<Float, Float> {
        float get(float x);

        default Float apply(Float x) {
            return x == null ? null : get(x);
        }
    }

    record Constant(float value) {}

    static Constant point(FloatFunction transformer, float value) {
        return new Constant((Float) transformer.apply(value));
    }

    static Integer kept(Function<String, Integer> lengths, String s) {
        return (Integer) lengths.apply(s);
    }

    public static void main(String[] args) {
        System.out.println(point(x -> x * 2, 1.5f));
        FloatFunction nulls = new FloatFunction() {
            public float get(float x) {
                return x;
            }

            public Float apply(Float x) {
                return null;
            }
        };
        try {
            System.out.println(point(nulls, 1f));
        } catch (NullPointerException e) {
            System.out.println("null: " + e.getMessage());
        }
        System.out.println(kept(String::length, "four") + " " + kept(s -> null, "none"));
    }
}
