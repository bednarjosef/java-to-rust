// The standard exception classes as the translator provides them: the constructors and the
// methods of `Throwable` that a translation can reproduce, in the subset of Java the translator
// supports. `build.rs` prints their syntax tree with `JavaTree`, and lowering gives each class
// its real package (`java.lang`, `java.util` or `java.io`) and adds the classes a program uses to it (see
// `lower/exceptions.rs`). Stack traces and serialization are left out; `cause` is null where
// Java's field holds the object itself to mean "not set yet", and `causeSet` tells the two apart
// for `initCause`. `addSuppressed` does not reject itself or null here, and `initCause` neither a
// second cause nor itself: this file cannot `throw`, since javac sees its `Throwable` as
// unrelated to `java.lang.Throwable`, so `lower/suppress.rs` adds the checks.
package standard;

class Throwable {
    private String message;
    private Throwable cause;
    private boolean causeSet;
    private Throwable[] suppressed = new Throwable[0];

    public Throwable() {
    }

    public Throwable(String message) {
        this.message = message;
    }

    public Throwable(String message, Throwable cause) {
        this.message = message;
        this.cause = cause;
        this.causeSet = true;
    }

    public Throwable(Throwable cause) {
        this.message = cause == null ? null : cause.toString();
        this.cause = cause;
        this.causeSet = true;
    }

    public String getMessage() {
        return message;
    }

    public String getLocalizedMessage() {
        return getMessage();
    }

    public Throwable getCause() {
        return cause;
    }

    // The message of the `IllegalStateException` for a cause set before, which
    // `lower/suppress.rs` throws when it is not null.
    public Throwable initCause(Throwable cause) {
        String overwrite = causeSet ? "Can't overwrite cause with " + (cause == null ? "a null" : cause.toString()) : null;
        this.cause = cause;
        this.causeSet = true;
        return this;
    }

    public void addSuppressed(Throwable exception) {
        Throwable[] own = suppressed;
        Throwable[] more = new Throwable[own.length + 1];
        for (int i = 0; i < more.length; i++) {
            more[i] = i < own.length ? own[i] : exception;
        }
        suppressed = more;
    }

    public Throwable[] getSuppressed() {
        Throwable[] own = suppressed;
        Throwable[] copy = new Throwable[own.length];
        for (int i = 0; i < own.length; i++) {
            copy[i] = own[i];
        }
        return copy;
    }

    public String toString() {
        String message = getLocalizedMessage();
        return message != null ? getClass().getName() + ": " + message : getClass().getName();
    }

    public void printStackTrace() {
        System.err.println(this);
        for (Throwable c = getCause(); c != null; c = c.getCause()) {
            System.err.println("Caused by: " + c);
        }
    }
}

class Exception extends Throwable {
    public Exception() {
    }

    public Exception(String message) {
        super(message);
    }

    public Exception(String message, Throwable cause) {
        super(message, cause);
    }

    public Exception(Throwable cause) {
        super(cause);
    }
}

class Error extends Throwable {
    public Error() {
    }

    public Error(String message) {
        super(message);
    }

    public Error(String message, Throwable cause) {
        super(message, cause);
    }

    public Error(Throwable cause) {
        super(cause);
    }
}

class LinkageError extends Error {
    public LinkageError() {
    }

    public LinkageError(String message, Throwable cause) {
        super(message, cause);
    }
}

class ExceptionInInitializerError extends LinkageError {
    public ExceptionInInitializerError(Throwable thrown) {
        super(null, thrown);
    }

    public ExceptionInInitializerError(String message) {
        super(message, null);
    }
}

class NoClassDefFoundError extends LinkageError {
    public NoClassDefFoundError() {
    }

    public NoClassDefFoundError(String message, Throwable cause) {
        super(message, cause);
    }
}

class AssertionError extends Error {
    public AssertionError() {
    }

    // Java's constructor takes the text of the object and, for a `Throwable`, makes it the cause.
    public AssertionError(Object detailMessage) {
        super(String.valueOf(detailMessage), detailMessage instanceof Throwable ? (Throwable) detailMessage : null);
    }

    public AssertionError(boolean detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(char detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(int detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(long detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(float detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(double detailMessage) {
        super(String.valueOf(detailMessage));
    }

    public AssertionError(String message, Throwable cause) {
        super(message, cause);
    }
}

class RuntimeException extends Exception {
    public RuntimeException() {
    }

    public RuntimeException(String message) {
        super(message);
    }

    public RuntimeException(String message, Throwable cause) {
        super(message, cause);
    }

    public RuntimeException(Throwable cause) {
        super(cause);
    }
}

class InterruptedException extends Exception {
    public InterruptedException() {
    }

    public InterruptedException(String s) {
        super(s);
    }
}

class CloneNotSupportedException extends Exception {
    public CloneNotSupportedException() {
    }

    public CloneNotSupportedException(String s) {
        super(s);
    }
}

class ArithmeticException extends RuntimeException {
    public ArithmeticException() {
    }

    public ArithmeticException(String s) {
        super(s);
    }
}

class ArrayStoreException extends RuntimeException {
    public ArrayStoreException() {
    }

    public ArrayStoreException(String s) {
        super(s);
    }
}

class ClassCastException extends RuntimeException {
    public ClassCastException() {
    }

    public ClassCastException(String s) {
        super(s);
    }
}

class IllegalArgumentException extends RuntimeException {
    public IllegalArgumentException() {
    }

    public IllegalArgumentException(String s) {
        super(s);
    }

    public IllegalArgumentException(String message, Throwable cause) {
        super(message, cause);
    }

    public IllegalArgumentException(Throwable cause) {
        super(cause);
    }
}

class IllegalThreadStateException extends IllegalArgumentException {
    public IllegalThreadStateException() {
    }

    public IllegalThreadStateException(String s) {
        super(s);
    }
}

class NumberFormatException extends IllegalArgumentException {
    public NumberFormatException() {
    }

    public NumberFormatException(String s) {
        super(s);
    }
}

class IllegalStateException extends RuntimeException {
    public IllegalStateException() {
    }

    public IllegalStateException(String s) {
        super(s);
    }

    public IllegalStateException(String message, Throwable cause) {
        super(message, cause);
    }

    public IllegalStateException(Throwable cause) {
        super(cause);
    }
}

class IllegalMonitorStateException extends RuntimeException {
    public IllegalMonitorStateException() {
    }

    public IllegalMonitorStateException(String s) {
        super(s);
    }

}

class IndexOutOfBoundsException extends RuntimeException {
    public IndexOutOfBoundsException() {
    }

    public IndexOutOfBoundsException(String s) {
        super(s);
    }

    public IndexOutOfBoundsException(int index) {
        super("Index out of range: " + index);
    }

    public IndexOutOfBoundsException(long index) {
        super("Index out of range: " + index);
    }
}

class ArrayIndexOutOfBoundsException extends IndexOutOfBoundsException {
    public ArrayIndexOutOfBoundsException() {
    }

    public ArrayIndexOutOfBoundsException(String s) {
        super(s);
    }

    public ArrayIndexOutOfBoundsException(int index) {
        super("Array index out of range: " + index);
    }
}

class StringIndexOutOfBoundsException extends IndexOutOfBoundsException {
    public StringIndexOutOfBoundsException() {
    }

    public StringIndexOutOfBoundsException(String s) {
        super(s);
    }

    public StringIndexOutOfBoundsException(int index) {
        super("String index out of range: " + index);
    }
}

class NegativeArraySizeException extends RuntimeException {
    public NegativeArraySizeException() {
    }

    public NegativeArraySizeException(String s) {
        super(s);
    }
}

class NullPointerException extends RuntimeException {
    public NullPointerException() {
    }

    public NullPointerException(String s) {
        super(s);
    }

    // Java's override describes a dereference that the runtime raises the exception for; the
    // analysis rejects reading that message, so this is the message the code gave.
    public String getMessage() {
        return super.getMessage();
    }
}

class UnsupportedOperationException extends RuntimeException {
    public UnsupportedOperationException() {
    }

    public UnsupportedOperationException(String message) {
        super(message);
    }

    public UnsupportedOperationException(String message, Throwable cause) {
        super(message, cause);
    }

    public UnsupportedOperationException(Throwable cause) {
        super(cause);
    }
}

class NoSuchElementException extends RuntimeException {
    public NoSuchElementException() {
    }

    public NoSuchElementException(String s) {
        super(s);
    }

    public NoSuchElementException(String s, Throwable cause) {
        super(s, cause);
    }

    public NoSuchElementException(Throwable cause) {
        super(cause);
    }
}

class ConcurrentModificationException extends RuntimeException {
    public ConcurrentModificationException() {
    }

    public ConcurrentModificationException(String message) {
        super(message);
    }

    public ConcurrentModificationException(Throwable cause) {
        super(cause);
    }

    public ConcurrentModificationException(String message, Throwable cause) {
        super(message, cause);
    }
}

class IOException extends Exception {
    public IOException() {
    }

    public IOException(String message) {
        super(message);
    }

    public IOException(String message, Throwable cause) {
        super(message, cause);
    }

    public IOException(Throwable cause) {
        super(cause);
    }
}

class FileNotFoundException extends IOException {
    public FileNotFoundException() {
    }

    public FileNotFoundException(String s) {
        super(s);
    }
}

class EOFException extends IOException {
    public EOFException() {
    }

    public EOFException(String s) {
        super(s);
    }
}

class UnsupportedEncodingException extends IOException {
    public UnsupportedEncodingException() {
    }

    public UnsupportedEncodingException(String s) {
        super(s);
    }
}

class UncheckedIOException extends RuntimeException {
    public UncheckedIOException(String message, IOException cause) {
        super(message, nonNull(cause));
    }

    // Java's `Objects.requireNonNull(cause)` of the constructor below: the dereference throws the
    // NullPointerException, and the message is `cause.toString()`, as `Throwable(Throwable)` sets it.
    public UncheckedIOException(IOException cause) {
        super(cause.toString(), cause);
    }

    public IOException getCause() {
        return (IOException) super.getCause();
    }

    private static IOException nonNull(IOException cause) {
        cause.toString();
        return cause;
    }
}

class SecurityException extends RuntimeException {
    public SecurityException() {
    }

    public SecurityException(String s) {
        super(s);
    }

    public SecurityException(String message, Throwable cause) {
        super(message, cause);
    }

    public SecurityException(Throwable cause) {
        super(cause);
    }
}

class ReflectiveOperationException extends Exception {
    public ReflectiveOperationException() {
    }

    public ReflectiveOperationException(String message) {
        super(message);
    }

    public ReflectiveOperationException(String message, Throwable cause) {
        super(message, cause);
    }

    public ReflectiveOperationException(Throwable cause) {
        super(cause);
    }
}

class ClassNotFoundException extends ReflectiveOperationException {
    public ClassNotFoundException() {
    }

    public ClassNotFoundException(String s) {
        super(s, null);
    }

    public ClassNotFoundException(String s, Throwable ex) {
        super(s, ex);
    }

    public Throwable getException() {
        return super.getCause();
    }
}

class NoSuchMethodException extends ReflectiveOperationException {
    public NoSuchMethodException() {
    }

    public NoSuchMethodException(String s) {
        super(s);
    }
}

class NoSuchFieldException extends ReflectiveOperationException {
    public NoSuchFieldException() {
    }

    public NoSuchFieldException(String s) {
        super(s);
    }
}

class IllegalAccessException extends ReflectiveOperationException {
    public IllegalAccessException() {
    }

    public IllegalAccessException(String s) {
        super(s);
    }
}

class InstantiationException extends ReflectiveOperationException {
    public InstantiationException() {
    }

    public InstantiationException(String s) {
        super(s);
    }
}

class InputMismatchException extends NoSuchElementException {
    public InputMismatchException() {
    }

    public InputMismatchException(String s) {
        super(s);
    }
}

class EmptyStackException extends RuntimeException {
    public EmptyStackException() {
    }
}

class MissingResourceException extends RuntimeException {
    private String className;
    private String key;

    public MissingResourceException(String s, String className, String key) {
        super(s);
        this.className = className;
        this.key = key;
    }

    public String getClassName() {
        return className;
    }

    public String getKey() {
        return key;
    }
}
