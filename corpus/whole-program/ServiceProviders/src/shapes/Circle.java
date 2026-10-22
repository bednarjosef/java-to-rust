package shapes;

public class Circle implements Shape {
    private static int made;

    public Circle() {
        made++;
        System.out.println("making circle " + made);
    }

    public String name() {
        return "circle";
    }

    public double area() {
        return 3.0;
    }
}
