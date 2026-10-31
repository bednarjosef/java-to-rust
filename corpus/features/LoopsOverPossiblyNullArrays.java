// Loops whose header reads the length of an array variable that may be null: the header throws
// Java's NullPointerException where the array is null, and otherwise the loop and the rest of the
// block read the array as not null. An index past the end still throws Java's
// ArrayIndexOutOfBoundsException.
public class LoopsOverPossiblyNullArrays {
    static final class Section {
        final short count;

        Section(int count) {
            this.count = (short) count;
        }

        boolean ticking() {
            return count > 0;
        }
    }

    static final class Chunk {
        private final Section[] sections;

        Chunk(int n) {
            sections = n < 0 ? null : new Section[n];
            for (int i = 0; i < n; i++) {
                sections[i] = new Section(i % 3);
            }
        }

        Section[] getSections() {
            return sections;
        }
    }

    static int[] numbers(int n) {
        if (n < 0) {
            return null;
        }
        int[] a = new int[n];
        for (int i = 0; i < n; i++) {
            a[i] = i * 7 % 5;
        }
        return a;
    }

    static int count(int[] a) {
        return a == null ? -1 : a.length;
    }

    static long tick(Chunk chunk, int speed) {
        long total = 0;
        if (speed > 0) {
            Section[] sections = chunk.getSections();
            for (int index = 0; index < sections.length; index++) {
                Section section = sections[index];
                if (section.ticking()) {
                    for (int i = 0; i < speed; i++) {
                        total += index * section.count;
                    }
                }
            }
            // The rest of the block reads the array as not null too.
            total += sections.length;
        }
        return total;
    }

    static int down(int n) {
        int[] a = numbers(n);
        int sum = 0;
        for (int i = a.length - 1; i >= 0; i--) {
            sum = sum * 3 + a[i];
        }
        return sum + count(a);
    }

    static int whileLoop(int n) {
        int[] a = numbers(n);
        int i = 0;
        int sum = 0;
        while (i < a.length) {
            sum += a[i];
            i += 2;
        }
        return sum;
    }

    static int calls;

    static int limit(int n) {
        calls++;
        return n;
    }

    // The header calls a method before it reads the length: the array is not bound before it.
    static int callFirst(int n) {
        int[] a = numbers(n);
        int sum = 0;
        for (int i = 0; i < limit(n) && i < a.length; i++) {
            sum += a[i];
        }
        return sum;
    }

    // One past the end.
    static int pastTheEnd(int n) {
        int[] a = numbers(n);
        int sum = 0;
        for (int i = 0; i <= a.length; i++) {
            sum += a[i];
        }
        return sum;
    }

    static void run(String name, java.util.function.IntSupplier body) {
        try {
            System.out.println(name + " " + body.getAsInt());
        } catch (RuntimeException e) {
            System.out.println(name + " " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        System.out.println(tick(new Chunk(24), 3));
        System.out.println(tick(new Chunk(0), 3));
        System.out.println(tick(new Chunk(-1), 0));
        try {
            System.out.println(tick(new Chunk(-1), 2));
        } catch (NullPointerException e) {
            System.out.println("tick " + e.getMessage());
        }
        for (int n : new int[] {5, 0, -1}) {
            run("down " + n, () -> down(n));
            run("while " + n, () -> whileLoop(n));
            run("callFirst " + n, () -> callFirst(n));
            run("pastTheEnd " + n, () -> pastTheEnd(n));
        }
        System.out.println("calls " + calls);
    }
}
