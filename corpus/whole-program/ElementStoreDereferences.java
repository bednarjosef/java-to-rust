/**
 * A field read on the value of an element store, `(h[k] = c).status`, dereferences the stored
 * value: one checked for null before cannot throw there, and one that may be null throws the
 * `NullPointerException` that a `catch` sees.
 */
public class ElementStoreDereferences {
    static final class Task {
        int status;

        Task(int status) {
            this.status = status;
        }
    }

    static int checked(Task[] h, Task c) {
        if (c == null) {
            return 0;
        }
        if ((h[0] = c).status >= 0) {
            return 1;
        }
        return 2;
    }

    static int unchecked(Task[] h, Task c) {
        return (h[0] = c).status;
    }

    public static void main(String[] args) {
        Task[] h = new Task[1];
        Task none = args.length > 3 ? new Task(0) : null;
        try {
            System.out.println(checked(h, new Task(1)));
            System.out.println(checked(h, new Task(-1)));
            System.out.println(checked(h, none));
            System.out.println(unchecked(h, new Task(7)));
            System.out.println(unchecked(h, none));
        } catch (NullPointerException e) {
            System.out.println("caught, stored " + h[0]);
        }
    }
}
