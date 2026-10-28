package factories;

/** An appender made by a generic builder, as log4j's appenders are. */
public class Appender {
    /** A builder whose type parameter is its own subclass, found by its static factory. */
    public static class Builder<B extends Builder<B>> {
        private String name = "unnamed";

        @SuppressWarnings("unchecked")
        public B asBuilder() {
            return (B) this;
        }

        public Appender build() {
            return new Appender(name);
        }
    }

    private final String name;

    Appender(String name) {
        this.name = name;
    }

    public String name() {
        return name;
    }

    public static <B extends Builder<B>> B newBuilder() {
        return new Builder<B>().asBuilder();
    }

    public static <T> T first(T[] items) {
        return items[0];
    }
}
