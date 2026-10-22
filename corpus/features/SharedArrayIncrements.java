// Incrementing an element of an array field of an object that threads share, where an index
// out of bounds is caught (as `DigitList.roundUp` does `++digits[maximumDigits]`): the update
// names the element's type, which nothing else in it tells.
public class SharedArrayIncrements {
    static final class Digits {
        final byte[] digits = new byte[4];
        final char[] letters = {'a', 'k'};
        final short[] counts = new short[2];

        int roundUp(int at) {
            while (true) {
                at--;
                if (at < 0) {
                    digits[0] = 1;
                    at = 0;
                    break;
                }
                ++digits[at];
                if (digits[at] <= 2) {
                    break;
                }
            }
            letters[at % 2]++;
            counts[at % 2]--;
            return at + 1;
        }

        String show() {
            return digits[0] + " " + digits[1] + " " + digits[2] + " " + digits[3] + " " + new String(letters)
                    + " " + counts[0] + " " + counts[1];
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Digits d = new Digits();
        Thread t = new Thread(() -> System.out.println("rounded to " + d.roundUp(3)));
        t.start();
        t.join();
        System.out.println(d.show());
        for (int i = 0; i < 4; i++) {
            System.out.println("rounded to " + d.roundUp(2));
        }
        System.out.println(d.show());
        try {
            d.roundUp(9);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(d.show());
    }
}
