// Final array fields of a class whose constructor starts a thread, so they are read through their
// cells: guards on the field's length bound the index (`i < this.sections.length`), counting loops
// over the field and over a field of one length read it, and an index past the end still throws
// ArrayIndexOutOfBoundsException.
public class FinalArrayFieldsAcrossThreads {
    static class Column {
        private final int[] sections;
        private final long[] recent = new long[4];
        private final Thread worker;

        Column(int height) {
            this.sections = new int[height];
            this.worker = new Thread(() -> {
                synchronized (this) {
                    for (int i = 0; i < this.sections.length; i++) {
                        this.sections[i] = i * 3;
                    }
                }
            });
            this.worker.start();
        }

        void await() throws InterruptedException {
            this.worker.join();
        }

        synchronized int at(int y) {
            if (y >= 0 && y < this.sections.length) {
                return this.sections[y];
            }
            return -1;
        }

        synchronized int total() {
            int sum = 0;
            for (int i = 0; i < this.sections.length; i++) {
                sum += this.sections[i];
            }
            return sum;
        }

        synchronized void remember(long value) {
            for (int i = 3; i > 0; i--) {
                this.recent[i] = this.recent[i - 1];
            }
            this.recent[0] = value;
        }

        synchronized long recentSum() {
            long sum = 0;
            for (int i = 0; i < 4; i++) {
                sum += this.recent[i] * (i + 1);
            }
            return sum;
        }

        synchronized int below(int y) {
            return this.sections[y - 1];
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Column column = new Column(6);
        column.await();
        for (long v = 1; v <= 6; v++) {
            column.remember(v * 10);
        }
        System.out.println(column.at(0) + " " + column.at(5) + " " + column.at(6) + " " + column.at(-1));
        System.out.println("total " + column.total() + " recent " + column.recentSum());
        System.out.println("below " + column.below(6));
        try {
            System.out.println(column.below(0));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("below: " + e.getMessage());
        }
    }
}
