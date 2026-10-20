public class NullWithClassUsed {
    static class Named { public String toString() { return "named"; } }
    public static void main(String[] args) {
        Named n = null;
        System.out.println("n=" + n);
        Named m = new Named();
        System.out.println(m);
    }
}
