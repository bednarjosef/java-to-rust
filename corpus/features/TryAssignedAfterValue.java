// Locals of a class declared without a value and assigned in a `try` block where that block cannot
// carry them out: in a loop's condition, after the code before the `try` has set them, as
// `Component.readObject` reads its keys; and by a statement of a `try` block with a `finally`,
// read after it, as `ThreadPoolExecutor.shutdownNow` takes its tasks.
public class TryAssignedAfterValue {
    static final Object END = "end";
    static int calls;

    static Object next(Object[] items) {
        if (calls >= items.length) throw new IllegalStateException("no item " + calls);
        return items[calls++];
    }

    static String keys(Object[] items) {
        calls = 0;
        StringBuilder sb = new StringBuilder();
        Object keyOrNull;
        while (!"end".equals(keyOrNull = next(items))) {
            sb.append(keyOrNull).append(';');
        }
        try {
            while (!"end".equals(keyOrNull = next(items))) {
                sb.append(((String) keyOrNull).length());
            }
        } catch (IllegalStateException e) {
            sb.append(" caught ").append(e.getMessage());
        }
        try {
            while (END != (keyOrNull = next(items))) {
                sb.append('+').append(keyOrNull);
            }
        } catch (IllegalStateException e) {
            sb.append(" again ").append(e.getMessage());
        }
        return sb.toString();
    }

    static String last(Object[] items) {
        calls = 0;
        Object item;
        item = next(items);
        String seen = "";
        try {
            while (!"end".equals(item = next(items))) {
                seen += item;
            }
        } catch (IllegalStateException e) {
            seen += "!";
        }
        return seen + " last " + item;
    }

    static class Tasks {
        String names;

        Tasks(String names) {
            this.names = names;
        }

        public String toString() {
            return "[" + names + "]";
        }
    }

    static int locks;

    static Tasks drain(Tasks queue, boolean fail) {
        Tasks tasks;
        locks++;
        try {
            if (fail) throw new IllegalStateException("cannot drain " + queue);
            tasks = new Tasks(queue.names);
            queue.names = "";
        } finally {
            locks--;
        }
        return tasks;
    }

    static String both(Tasks queue, boolean fail) {
        Tasks tasks;
        String note;
        try {
            if (fail) throw new IllegalStateException("failed");
            tasks = new Tasks(queue.names);
            note = "copied";
        } catch (IllegalStateException e) {
            tasks = new Tasks("");
            note = e.getMessage();
        } finally {
            locks++;
        }
        return note + " " + tasks;
    }

    public static void main(String[] args) {
        System.out.println(keys(new Object[] {"a", "b", "end", "cc", "ddd", "end", "x"}));
        System.out.println(keys(new Object[] {"a", "end", "x"}));
        System.out.println(last(new Object[] {"a", "b", "c", "end"}));
        System.out.println(last(new Object[] {"a", "b"}));
        Tasks queue = new Tasks("one, two");
        System.out.println(drain(queue, false) + " " + queue + " " + locks);
        try {
            drain(queue, true);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage() + " " + locks);
        }
        System.out.println(both(new Tasks("x"), false) + " " + locks);
        System.out.println(both(new Tasks("x"), true) + " " + locks);
    }
}
