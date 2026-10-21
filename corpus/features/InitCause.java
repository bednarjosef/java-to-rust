// Throwable.initCause sets a cause once: a second cause, or one given to the constructor, is an
// IllegalStateException, and a throwable is not its own cause.
public class InitCause {
    static class Failure extends Exception {
        Failure(String message) {
            super(message);
        }
    }

    public static void main(String[] args) {
        Failure failure = new Failure("outer");
        Throwable same = failure.initCause(new IllegalArgumentException("inner"));
        System.out.println(same == failure);
        System.out.println(failure.getCause().getMessage());
        try {
            failure.initCause(null);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
        Failure lone = new Failure("lone");
        try {
            lone.initCause(lone);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        lone.initCause(null);
        System.out.println(lone.getCause());
        try {
            lone.initCause(new RuntimeException("late"));
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
        RuntimeException wrapped = new RuntimeException("wrapped", failure);
        try {
            wrapped.initCause(failure);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
    }
}
