// A builder field that a thread reaches, appended to a range of a `CharSequence` that a program
// class implements, as `NormalizerImpl.ReorderingBuffer.append` does. A null sequence appends a
// range of "null"; a range that fails is checked on the sequence's length, and what its `charAt`
// throws goes on from the append; a null builder field throws NullPointerException.
public class SharedBuilderRangeAppend {
    static class Letters implements CharSequence {
        final String text;

        Letters(String text) {
            this.text = text;
        }

        public int length() {
            return text.length();
        }

        public char charAt(int i) {
            if (text.charAt(i) == '!') {
                throw new IllegalArgumentException("no ! at " + i);
            }
            return text.charAt(i);
        }

        public CharSequence subSequence(int start, int end) {
            return new Letters(text.substring(start, end));
        }

        @Override
        public String toString() {
            return text;
        }
    }

    static class Buffer {
        StringBuilder str;
        int reorderStart;
        int lastCC;

        void append(CharSequence s, int start, int limit) {
            if (start == limit) {
                return;
            }
            if (lastCC <= 1) {
                reorderStart = str.length() + (limit - start);
            }
            str.append(s, start, limit);
            lastCC = 1;
        }

        void appendOnly(CharSequence s, int start, int limit) {
            str.append(s, start, limit);
        }
    }

    static String attempt(Buffer buffer, CharSequence s, int start, int limit) {
        try {
            buffer.append(s, start, limit);
            return "ok " + buffer.str;
        } catch (IllegalArgumentException | IndexOutOfBoundsException e) {
            return e.getClass().getSimpleName() + ": " + e.getMessage() + " in " + buffer.str;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Buffer buffer = new Buffer();
        buffer.str = new StringBuilder("ab");
        Letters letters = new Letters("hello!world");

        Thread worker = new Thread(() -> {
            buffer.append(new Letters("xyz"), 0, 2);
            buffer.append(null, 1, 3);
            buffer.append(null, 0, 4);
        });
        worker.start();
        worker.join();
        System.out.println(buffer.str + " " + buffer.reorderStart + " " + buffer.lastCC);

        System.out.println(attempt(buffer, letters, 0, 5));
        System.out.println(attempt(buffer, letters, 2, 9));
        System.out.println(attempt(buffer, null, 0, 5));
        System.out.println(attempt(buffer, null, 3, 3));
        System.out.println(attempt(buffer, new Letters("abc"), 2, 1));
        System.out.println(attempt(buffer, new Letters("abc"), 0, 4));
        System.out.println(buffer.str.length() + " " + buffer.reorderStart);

        Buffer empty = new Buffer();
        try {
            empty.append(letters, 0, 3);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
        try {
            empty.appendOnly(letters, 0, 3);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
        try {
            empty.appendOnly(null, 0, 4);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
        Buffer late = new Buffer();
        late.str = new StringBuilder();
        Thread reader = new Thread(() -> late.appendOnly(new Letters("!"), 0, 1));
        reader.start();
        reader.join();
        System.out.println("[" + late.str + "] after the thread's failure");
    }
}
