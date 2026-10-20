public class MathExactOverflow {
    public static void main(String[] args) {
        // The results in range print; the last call overflows a long, which throws and ends the program.
        System.out.println(Math.addExact(Integer.MAX_VALUE, -1) + " " + Math.subtractExact(Integer.MIN_VALUE, -1));
        System.out.println(Math.multiplyExact(46340, 46340) + " " + Math.negateExact(Integer.MIN_VALUE + 1));
        System.out.println(Math.incrementExact(Integer.MAX_VALUE - 1) + " " + Math.toIntExact(Integer.MAX_VALUE));
        System.out.println(Math.multiplyExact(Long.MAX_VALUE, 0L));
        System.out.println(Math.multiplyExact(Long.MAX_VALUE, 2));
        System.out.println("not reached");
    }
}
