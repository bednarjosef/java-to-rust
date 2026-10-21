// The name of the JDK's `ThreadLocal.ThreadLocalMap`, for `java/Threads.java` to declare the fields
// that `ThreadLocal` keeps its values in. Only `Threads.java`'s own classes are the translator's
// (`lower/exceptions.rs` takes the first unit's): this one is compiled with it and left out, and
// its name becomes the JDK's, whose class the program uses.
package threads;

class ThreadLocal {
    static class ThreadLocalMap {
    }
}
