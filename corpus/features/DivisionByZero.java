public class DivisionByZero {
    static int divide(int a, int b) {
        return a / b;
    }

    public static void main(String[] args) {
        System.out.println(divide(10, 3));
        System.out.print("about to fail... ");
        System.out.println(divide(1, 0));
        System.out.println("never printed");
    }
}
