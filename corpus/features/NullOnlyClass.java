public class NullOnlyClass {
    static class Named { int x; }
    public static void main(String[] args) {
        Named n = null;
        System.out.println(n == null);
    }
}
