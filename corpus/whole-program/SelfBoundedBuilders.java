// A builder whose type parameter is bounded by itself, as log4j's appenders' builders are
// (`<B extends Builder<B>> B newBuilder()`): called with no type to infer `B` from, javac makes
// up a type variable for it, whose values are of its bound.
public class SelfBoundedBuilders {
    static class Appender {
        final String name;
        final boolean follow;

        Appender(String name, boolean follow) {
            this.name = name;
            this.follow = follow;
        }

        static class Builder<B extends Builder<B>> {
            String name = "default";
            boolean follow;

            @SuppressWarnings("unchecked")
            B asBuilder() {
                return (B) this;
            }

            B setName(String name) {
                this.name = name;
                return asBuilder();
            }

            B setFollow(boolean follow) {
                this.follow = follow;
                return asBuilder();
            }

            Appender build() {
                return new Appender(name, follow);
            }
        }

        static <B extends Builder<B>> B newBuilder() {
            return new Builder<B>().asBuilder();
        }
    }

    public static void main(String[] args) {
        Appender console = Appender.newBuilder().setName("console").setFollow(true).build();
        System.out.println(console.name + " " + console.follow);
        var builder = Appender.newBuilder();
        System.out.println(builder.setName("file").build().name + " " + builder.follow);
    }
}
