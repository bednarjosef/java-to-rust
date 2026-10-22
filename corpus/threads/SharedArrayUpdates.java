// Two arrays that a thread reaches, a static `int[]` and a static `char[]`, are shared, so the
// methods that the main thread calls with them take the shared form. Those methods update an
// element in place (`a[i] <<= n`, `a[i]++`, `a[i] /= d`) and write code point units into the char
// array (`Character.toChars`). The exceptions of those operations reach a `catch` in main, so they
// are returned to the caller rather than ending the program.
public class SharedArrayUpdates {
    static int[] counts = new int[4];
    static char[] units = new char[6];

    static int shiftTop(int[] a, int len, int n) {
        a[len - 1] <<= n;
        return a[len - 2]++;
    }

    static int divideAt(int[] a, int i, int d) {
        a[i] /= d;
        return a[i];
    }

    static int bumpAt(int[] a, int i) {
        return a[i]++;
    }

    static int writeUnits(char[] dst, int codePoint, int index) {
        return Character.toChars(codePoint, dst, index);
    }

    public static void main(String[] args) throws InterruptedException {
        Thread worker = new Thread(() -> {
            for (int i = 0; i < counts.length; i++) {
                counts[i] = i + 1;
            }
            units[0] = 'w';
            writeUnits(units, 0x1F600, 1);
        });
        worker.start();
        worker.join();
        int before = shiftTop(counts, counts.length, 3);
        int written = writeUnits(units, 0x41, 3);
        try {
            shiftTop(counts, 0, 1);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("shift: " + e.getMessage());
        }
        try {
            divideAt(counts, 1, 0);
        } catch (ArithmeticException e) {
            System.out.println("divide: " + e.getMessage());
        }
        try {
            bumpAt(counts, -1);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("bump: " + e.getMessage());
        }
        try {
            writeUnits(units, 0x110000, 4);
        } catch (IllegalArgumentException e) {
            System.out.println("write: " + e.getMessage());
        }
        int halved = divideAt(counts, 0, 2);
        StringBuilder line = new StringBuilder();
        for (int i = 0; i < counts.length; i++) {
            line.append(counts[i]).append(' ');
        }
        System.out.println(line.toString().trim());
        System.out.println("before " + before + " written " + written + " halved " + halved);
        StringBuilder codes = new StringBuilder();
        for (char unit : units) {
            codes.append((int) unit).append(' ');
        }
        System.out.println(codes.toString().trim());
    }
}
