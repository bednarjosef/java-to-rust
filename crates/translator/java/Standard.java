// The standard exception classes as the translator provides them: the constructors and the
// methods of `Throwable` that a translation can reproduce, in the subset of Java the translator
// supports. `build.rs` prints their syntax tree with `JavaTree`, and lowering gives each class
// its real package (`java.lang`, `java.util` or `java.io`) and adds the classes a program uses to it (see
// `lower/exceptions.rs`). Stack traces and serialization are left out; `cause` is null where
// Java's field holds the object itself to mean "not set yet", which only `initCause` tells apart.
// `addSuppressed` does not reject itself or null: this file cannot `throw`, since javac sees its
// `Throwable` as unrelated to `java.lang.Throwable`.
package standard;

class Throwable {
    private String message;
    private Throwable cause;
    private Throwable[] suppressed = new Throwable[0];

    public Throwable() {
    }

    public Throwable(String message) {
        this.message = message;
    }

    public Throwable(String message, Throwable cause) {
        this.message = message;
        this.cause = cause;
    }

    public Throwable(Throwable cause) {
        this.message = cause == null ? null : cause.toString();
        this.cause = cause;
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

class AssertionError extends Error {
    public AssertionError() {
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
