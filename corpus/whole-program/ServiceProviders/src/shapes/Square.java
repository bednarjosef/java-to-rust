package shapes;

public class Square implements Shape {
    private final String how;

    public Square() {
        this("by its constructor");
    }

    private Square(String how) {
        this.how = how;
    }

    // A provider on the class path is made by its constructor: this is not called.
    public static Square provider() {
        return new Square("by provider()");
    }

    public String name() {
        return "square " + how;
    }

    public double area() {
        return 4.0;
    }
}
