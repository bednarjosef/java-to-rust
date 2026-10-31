// Guarded indexes of arrays that live in fields and are read through a local variable or a
// parameter: the guards keep them in bounds as they do for a method's own arrays, while an index
// past the end, or a variable given another array after its guard, still throws.
public class HeapArrayBoundsGuards {
    static class Section {
        final int blocks;

        Section(int blocks) {
            this.blocks = blocks;
        }

        boolean ticking() {
            return blocks % 3 != 0;
        }
    }

    static class Chunk {
        final Section[] sections;
        int[] heights = new int[6];

        Chunk(int count) {
            sections = new Section[count];
            for (int i = 0; i < count; i++) {
                sections[i] = i % 4 == 1 ? null : new Section(i * 7);
            }
        }

        Section[] sections() {
            return sections;
        }
    }

    static int ticking(Chunk chunk) {
        Section[] sections = chunk.sections();
        int count = 0;
        for (int i = 0; i < sections.length; i++) {
            Section section = sections[i];
            if (section != null && section.ticking()) {
                count += section.blocks;
            }
        }
        return count;
    }

    static void raise(Chunk chunk, int by) {
        int[] heights = chunk.heights;
        for (int i = 0; i < heights.length; i++) {
            heights[i] = heights[i] + by * i;
        }
    }

    static int sumFrom(int[] values, int from) {
        int sum = 0;
        int i = from;
        while (i >= 0 && i < values.length) {
            sum += values[i];
            i += 2;
        }
        return sum;
    }

    static int lastPair(long[] values) {
        int n = values.length;
        int found = -1;
        for (int i = 0; i + 1 < n; i++) {
            if (values[i] < values[i + 1]) {
                found = i;
            }
        }
        return found;
    }

    static int[] filled;

    static int made(int n) {
        int[] squares = new int[n];
        for (int i = 0; i < n; i++) {
            squares[i] = i * i;
        }
        filled = squares;
        return squares[n - 1];
    }

    static int past(int[] values) {
        int sum = 0;
        for (int i = 0; i <= values.length; i++) {
            sum += values[i];
        }
        return sum;
    }

    static int swapped(int[] values, int[] other) {
        int sum = 0;
        int[] a = values;
        for (int i = 0; i < a.length; i++) {
            if (i == 2) {
                a = other;
            }
            sum += a[i];
        }
        return sum;
    }

    public static void main(String[] args) {
        Chunk chunk = new Chunk(10);
        System.out.println("ticking " + ticking(chunk));
        raise(chunk, 3);
        raise(chunk, 1);
        System.out.println("heights " + java.util.Arrays.toString(chunk.heights));
        System.out.println("from 1 " + sumFrom(chunk.heights, 1) + ", from -1 " + sumFrom(chunk.heights, -1));
        System.out.println("pair " + lastPair(new long[] {5, 3, 4, 9, 2}) + " " + lastPair(new long[0]));
        System.out.println("made " + made(7) + " " + filled.length);
        try {
            System.out.println(past(chunk.heights));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past: " + e.getMessage());
        }
        try {
            System.out.println(swapped(new int[] {1, 2, 3, 4, 5}, new int[] {10, 20, 30}));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("swapped: " + e.getMessage());
        }
        try {
            System.out.println(made(0));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("made 0: " + e.getMessage());
        }
    }
}
