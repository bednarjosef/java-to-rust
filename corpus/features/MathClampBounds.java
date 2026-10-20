public class MathClampBounds {
    public static void main(String[] args) {
        // Bounds in order: the results print. The last call has its bounds out of order, which throws.
        System.out.println(Math.clamp(1L, 1, 1) + " " + Math.clamp(9.5f, 0.0f, 9.5f) + " " + Math.clamp(Long.MIN_VALUE, -1L, 1L));
        System.out.println(Math.clamp(0.0, -0.0, 0.0) + " " + Math.clamp(-0.0f, -0.0f, 0.0f));
        System.out.println(Math.clamp(3L, 3, 2));
        System.out.println("not reached");
    }
}
