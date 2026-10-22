// The standard exception classes as the translator provides them: the constructors and the
// methods of `Throwable` that a translation can reproduce, in the subset of Java the translator
// supports. `build.rs` prints their syntax tree with `JavaTree`, and lowering gives each class
// its real package (`java.lang`, `java.util` or `java.io`) and adds the classes a program uses to it (see
// `lower/exceptions.rs`). Serialization is left out, and so are stack frames: a stack trace is
// empty unless `setStackTrace` gave it frames, as the specification of `getStackTrace` allows, and
// `printStackTrace` prints what Java prints with that trace. `cause` is null where Java's field
// holds the object itself to mean "not set yet", and `causeSet` tells the two apart for
// `initCause`. `addSuppressed` does not reject itself or null here, `initCause` neither a second
// cause nor itself, and `setStackTrace` and `StackTraceElement` no null: this file cannot `throw`,
// since javac sees its `Throwable` as unrelated to `java.lang.Throwable`, so `lower/suppress.rs`
// adds the checks.
package standard;

class Throwable {
    private String message;
    private Throwable cause;
    private boolean causeSet;
    // Null until an exception is suppressed, or until `setStackTrace` gives frames.
    private Throwable[] suppressed;
    private StackTraceElement[] stackTrace;

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
        Throwable[] own = ownSuppressed();
        Throwable[] more = new Throwable[own.length + 1];
        for (int i = 0; i < more.length; i++) {
            more[i] = i < own.length ? own[i] : exception;
        }
        suppressed = more;
    }

    public Throwable[] getSuppressed() {
        Throwable[] own = ownSuppressed();
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

    public StackTraceElement[] getStackTrace() {
        StackTraceElement[] own = ourStackTrace();
        StackTraceElement[] copy = new StackTraceElement[own.length];
        for (int i = 0; i < own.length; i++) {
            copy[i] = own[i];
        }
        return copy;
    }

    // `missing` names the first null frame, for the `NullPointerException` that
    // `lower/suppress.rs` throws when it is not null.
    public void setStackTrace(StackTraceElement[] stackTrace) {
        String missing = nullFrame(stackTrace);
        StackTraceElement[] copy = new StackTraceElement[stackTrace.length];
        for (int i = 0; i < copy.length; i++) {
            copy[i] = stackTrace[i];
        }
        this.stackTrace = copy;
    }

    private static String nullFrame(StackTraceElement[] stackTrace) {
        for (int i = 0; i < stackTrace.length; i++) {
            if (stackTrace[i] == null) {
                return "stackTrace[" + i + "]";
            }
        }
        return null;
    }

    // There are no frames to record.
    public Throwable fillInStackTrace() {
        return this;
    }

    // The fields' arrays, or empty ones while they are null (the null tests are of locals, which
    // the nullness analysis follows).
    private StackTraceElement[] ourStackTrace() {
        StackTraceElement[] own = stackTrace;
        return own != null ? own : new StackTraceElement[0];
    }

    private Throwable[] ownSuppressed() {
        Throwable[] own = suppressed;
        return own != null ? own : new Throwable[0];
    }

    // Java prints the lines of the trace with one `println` each; the text of them all with one
    // is the same, as they end with the line separator.
    public void printStackTrace() {
        System.err.println(stackTraceText());
    }

    public void printStackTrace(java.io.PrintStream s) {
        s.println(stackTraceText());
    }

    public void printStackTrace(java.io.PrintWriter s) {
        s.println(stackTraceText());
    }

    // What `printStackTrace` prints, without the last line separator: Java's `printStackTrace`
    // and `printEnclosedTrace`. The suppressed exceptions are read from the field, as Java's
    // `getSuppressed` is final.
    private String stackTraceText() {
        StringBuilder out = new StringBuilder();
        out.append(String.valueOf(this));
        StackTraceElement[] trace = ourStackTrace();
        for (int i = 0; i < trace.length; i++) {
            out.append("\n\tat " + trace[i]);
        }
        Throwable[] seen = new Throwable[] {this};
        Throwable[] own = ownSuppressed();
        for (int i = 0; i < own.length; i++) {
            seen = own[i].enclosedTrace(out, trace, "Suppressed: ", "\t", seen);
        }
        Throwable cause = getCause();
        if (cause != null) {
            seen = cause.enclosedTrace(out, trace, "Caused by: ", "", seen);
        }
        return out.toString();
    }

    // Appends the trace of an exception that another one's trace holds, as a cause or as
    // suppressed, with the frames that it has in common with that one's `enclosing` elided; the
    // exceptions printed so far are `seen`, and the result adds those printed here.
    private Throwable[] enclosedTrace(
            StringBuilder out, StackTraceElement[] enclosing, String caption, String prefix, Throwable[] seen) {
        for (int i = 0; i < seen.length; i++) {
            if (seen[i] == this) {
                out.append("\n" + prefix + caption + "[CIRCULAR REFERENCE: " + this + "]");
                return seen;
            }
        }
        Throwable[] printed = new Throwable[seen.length + 1];
        for (int i = 0; i < printed.length; i++) {
            printed[i] = i < seen.length ? seen[i] : this;
        }
        StackTraceElement[] trace = ourStackTrace();
        int m = trace.length - 1;
        int n = enclosing.length - 1;
        while (m >= 0 && n >= 0 && trace[m].sameFrame(enclosing[n])) {
            m--;
            n--;
        }
        int framesInCommon = trace.length - 1 - m;
        out.append("\n" + prefix + caption + this);
        for (int i = 0; i <= m; i++) {
            out.append("\n" + prefix + "\tat " + trace[i]);
        }
        if (framesInCommon != 0) {
            out.append("\n" + prefix + "\t... " + framesInCommon + " more");
        }
        Throwable[] own = ownSuppressed();
        for (int i = 0; i < own.length; i++) {
            printed = own[i].enclosedTrace(out, trace, "Suppressed: ", prefix + "\t", printed);
        }
        Throwable cause = getCause();
        if (cause != null) {
            printed = cause.enclosedTrace(out, trace, "Caused by: ", prefix, printed);
        }
        return printed;
    }

    // What the runtime prints after `Exception in thread "main" ` for an exception that nothing
    // catches, which Java's thread group prints with `printStackTrace(System.err)`: the runtime
    // has this once `installTrace0()` has run (see `lower/natives.rs`), and `uncaught0()` is the
    // exception.
    private static String uncaughtTrace() {
        return uncaught0().stackTraceText();
    }

    private static native Throwable uncaught0();
}

// A frame of a stack trace, which only `setStackTrace` gives an exception: the class, the method,
// the file and the line, without Java's class loader and module.
final class StackTraceElement {
    private String declaringClass;
    private String methodName;
    private String fileName;
    private int lineNumber;

    // `missing` is the message of the `NullPointerException` that `lower/suppress.rs` throws when
    // it is not null, as Java's `Objects.requireNonNull` of the two names.
    public StackTraceElement(String declaringClass, String methodName, String fileName, int lineNumber) {
        String missing =
                declaringClass == null ? "Declaring class is null" : methodName == null ? "Method name is null" : null;
        this.declaringClass = declaringClass;
        this.methodName = methodName;
        this.fileName = fileName;
        this.lineNumber = lineNumber;
    }

    public String getClassName() {
        return declaringClass;
    }

    public String getMethodName() {
        return methodName;
    }

    public String getFileName() {
        return fileName;
    }

    public int getLineNumber() {
        return lineNumber;
    }

    public boolean isNativeMethod() {
        return lineNumber == -2;
    }

    public String toString() {
        String place;
        if (isNativeMethod()) {
            place = "Native Method";
        } else if (fileName == null) {
            place = "Unknown Source";
        } else if (lineNumber >= 0) {
            place = fileName + ":" + lineNumber;
        } else {
            place = fileName;
        }
        return declaringClass + "." + methodName + "(" + place + ")";
    }

    // Java's `equals` of two elements, which `Throwable` compares (it has no `equals(Object)`).
    boolean sameFrame(StackTraceElement e) {
        return e.lineNumber == lineNumber
                && e.declaringClass.equals(declaringClass)
                && (methodName == null ? e.methodName == null : methodName.equals(e.methodName))
                && (fileName == null ? e.fileName == null : fileName.equals(e.fileName));
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

    public LinkageError(String message) {
        super(message);
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

    public NoClassDefFoundError(String message) {
        super(message, null);
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

// The exception of a method or constructor that reflection calls, which Java gives as the
// cause: `getCause()` and `getTargetException()` are it, and the message is null.
class InvocationTargetException extends ReflectiveOperationException {
    private final Throwable target;

    protected InvocationTargetException() {
        super((String) null, null);
        this.target = null;
    }

    public InvocationTargetException(Throwable target) {
        super((String) null, target);
        this.target = target;
    }

    public InvocationTargetException(Throwable target, String s) {
        super(s, target);
        this.target = target;
    }

    public Throwable getTargetException() {
        return target;
    }

    public Throwable getCause() {
        return target;
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

    MissingResourceException(String message, String className, String key, Throwable cause) {
        super(message, cause);
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

// A stack walker, which a translation has only to ask for the class of its caller: lowering
// answers `getCallerClass()` where it is called, of a walker made with `RETAIN_CLASS_REFERENCE`
// (see `lower/callers.rs`), so this class has nothing to hold. Its other methods walk frames,
// which a translation does not have, and are rejected.
final class StackWalker {
    public enum Option {
        RETAIN_CLASS_REFERENCE,
        SHOW_REFLECT_FRAMES,
        SHOW_HIDDEN_FRAMES
    }

    private StackWalker() {
    }

    public static StackWalker getInstance() {
        return new StackWalker();
    }

    public static StackWalker getInstance(Option option) {
        return new StackWalker();
    }
}
