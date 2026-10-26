// A `for` loop whose condition is `true` as a method's last statement, which only a
// `return` or an exception leaves (`java.awt.font.TextLayout`'s `findRunContaining`).
public class ForLoopWithTrueCondition {
    static int runContaining(int index, int[] starts) {
        for (int i = 1; true; i++) {
            if (starts[i] > index) {
                return i - 1;
            }
        }
    }

    public static void main(String[] args) {
        int[] starts = {0, 3, 7, 12};
        System.out.println(runContaining(5, starts) + " " + runContaining(0, starts));
        try {
            System.out.println(runContaining(20, starts));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the end");
        }
    }
}
