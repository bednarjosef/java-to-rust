/** A local whose first assignment assigns it again inside its value, which Java allows. */
public class AssignedInFirstValue {
    enum Mode { ADD, REMOVE }

    static Mode pick(String mode) {
        Mode op = mode == null ? op = Mode.ADD : Mode.valueOf(mode);
        return op;
    }

    public static void main(String[] args) {
        System.out.println(pick(null));
        System.out.println(pick("REMOVE"));
        try {
            pick("MULTIPLY");
        } catch (IllegalArgumentException e) {
            System.out.println("no mode MULTIPLY");
        }
        int n = args.length > 3 ? (n = 4) + n : 7;
        System.out.println(n);
    }
}
