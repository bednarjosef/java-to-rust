import java.util.function.Function;

public class ChainReferenceMessages {
    static class Box {
        final boolean last;

        Box(boolean last) {
            this.last = last;
        }

        Box with(Function<String, String> f) {
            return last ? null : new Box(true);
        }
    }

    static class FirstBox extends Box {
        FirstBox() {
            super(false);
        }
    }

    public static void main(String[] args) {
        try {
            new FirstBox()
                .with(String::trim)
                .with(String::strip)
                .with(s -> s);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            new FirstBox().with(String::trim).with(String::strip).with(String::strip);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
