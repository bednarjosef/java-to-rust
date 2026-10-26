// The name of the JDK's `ThreadLocal.ThreadLocalMap`, for `java/Threads.java` to declare the fields
// that `ThreadLocal` keeps its values in, and what a new thread calls to copy its parent's
// inheritable ones. Only `Threads.java`'s own classes are the translator's
// (`lower/exceptions.rs` takes the first unit's): this one is compiled with it and left out, and
// its name becomes the JDK's, whose class the program uses.
package threads;

class ThreadLocal {
    static ThreadLocalMap createInheritedMap(ThreadLocalMap parentMap) {
        return null;
    }

    static class ThreadLocalMap {
        int size() {
            return 0;
        }
    }
}

// The translator's patch of `Module` (`java/patches/java/lang/Module.java`), whose class path the
// runtime's `ClassLoader` reads resources from.
class Module {
    static java.util.ArrayList<java.net.URL> classPathResources(String name, boolean all) {
        return null;
    }

    static java.io.InputStream openFile(java.net.URL url) throws java.io.IOException {
        return null;
    }
}
