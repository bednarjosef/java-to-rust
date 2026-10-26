import java.util.Objects;

// A field whose initializer reads a constant that is null, as gson's `GsonBuilder.strictness`
// starts as `DEFAULT_STRICTNESS`, which is `null`, until a setter gives it a value that is not.
public class FieldInitializedFromANullConstant {
    enum Strictness { LENIENT, STRICT }

    static class Builder {
        private static final Strictness DEFAULT_STRICTNESS = null;

        Strictness strictness = DEFAULT_STRICTNESS;
        boolean serializeNulls;

        Builder() {}

        Builder(Builder other) {
            this.strictness = other.strictness;
        }

        Builder setStrictness(Strictness strictness) {
            this.strictness = Objects.requireNonNull(strictness);
            return this;
        }
    }

    public static void main(String[] args) {
        Builder plain = new Builder();
        System.out.println(plain.strictness + " " + plain.serializeNulls);
        Builder strict = new Builder().setStrictness(Strictness.STRICT);
        System.out.println(strict.strictness + " " + new Builder(strict).strictness);
        System.out.println(new Builder(plain).strictness == null);
    }
}
