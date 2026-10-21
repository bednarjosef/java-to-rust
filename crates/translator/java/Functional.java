// The functional interfaces of the JDK as the translator provides them to a program that is
// translated without the JDK's sources: their abstract methods, declared as the JDK declares them
// (the same type variables, so that javac's signatures of the real ones name these). `build.rs`
// prints their syntax tree with `JavaTree`, and lowering gives each interface its real package
// (`java.lang`, `java.util`, `java.util.concurrent` or `java.util.function`) and adds the ones a
// program uses to it (see `lower/functional.rs`). Default and static methods need the JDK's
// sources: a program translated with them gets the JDK's own interfaces instead.
package functional;

interface Runnable {
    void run();
}

interface Callable<V> {
    V call();
}

interface Comparator<T> {
    int compare(T o1, T o2);
}

interface Iterable<T> {
    Iterator<T> iterator();
}

interface Iterator<E> {
    boolean hasNext();

    E next();
}

interface Function<T, R> {
    R apply(T t);
}

interface BiFunction<T, U, R> {
    R apply(T t, U u);
}

interface UnaryOperator<T> extends Function<T, T> {
}

interface BinaryOperator<T> extends BiFunction<T, T, T> {
}

interface Supplier<T> {
    T get();
}

interface Consumer<T> {
    void accept(T t);
}

interface BiConsumer<T, U> {
    void accept(T t, U u);
}

interface Predicate<T> {
    boolean test(T t);
}

interface BiPredicate<T, U> {
    boolean test(T t, U u);
}

interface BooleanSupplier {
    boolean getAsBoolean();
}

interface IntFunction<R> {
    R apply(int value);
}

interface IntPredicate {
    boolean test(int value);
}

interface IntUnaryOperator {
    int applyAsInt(int operand);
}

interface IntBinaryOperator {
    int applyAsInt(int left, int right);
}

interface IntConsumer {
    void accept(int value);
}

interface IntSupplier {
    int getAsInt();
}

interface IntToLongFunction {
    long applyAsLong(int value);
}

interface IntToDoubleFunction {
    double applyAsDouble(int value);
}

interface LongFunction<R> {
    R apply(long value);
}

interface LongPredicate {
    boolean test(long value);
}

interface LongUnaryOperator {
    long applyAsLong(long operand);
}

interface LongBinaryOperator {
    long applyAsLong(long left, long right);
}

interface LongConsumer {
    void accept(long value);
}

interface LongSupplier {
    long getAsLong();
}

interface LongToIntFunction {
    int applyAsInt(long value);
}

interface LongToDoubleFunction {
    double applyAsDouble(long value);
}

interface DoubleFunction<R> {
    R apply(double value);
}

interface DoublePredicate {
    boolean test(double value);
}

interface DoubleUnaryOperator {
    double applyAsDouble(double operand);
}

interface DoubleBinaryOperator {
    double applyAsDouble(double left, double right);
}

interface DoubleConsumer {
    void accept(double value);
}

interface DoubleSupplier {
    double getAsDouble();
}

interface DoubleToIntFunction {
    int applyAsInt(double value);
}

interface DoubleToLongFunction {
    long applyAsLong(double value);
}

interface ToIntFunction<T> {
    int applyAsInt(T value);
}

interface ToLongFunction<T> {
    long applyAsLong(T value);
}

interface ToDoubleFunction<T> {
    double applyAsDouble(T value);
}

interface ToIntBiFunction<T, U> {
    int applyAsInt(T t, U u);
}

interface ToLongBiFunction<T, U> {
    long applyAsLong(T t, U u);
}

interface ToDoubleBiFunction<T, U> {
    double applyAsDouble(T t, U u);
}

interface ObjIntConsumer<T> {
    void accept(T t, int value);
}

interface ObjLongConsumer<T> {
    void accept(T t, long value);
}

interface ObjDoubleConsumer<T> {
    void accept(T t, double value);
}
