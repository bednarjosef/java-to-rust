import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;

// A constructor's method handle, made once in a static block and invoked to make constants, as
// the JDK's ExtendedMapMode makes FileChannel.MapMode's extra modes.
public class ConstructorHandles {
    static final class Mode {
        static final Mode PLAIN = new Mode("PLAIN");

        private final String name;

        private Mode(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return "Mode " + name;
        }
    }

    static final MethodHandle MAKE;

    static {
        try {
            MethodHandles.Lookup lookup = MethodHandles.privateLookupIn(Mode.class, MethodHandles.lookup());
            var type = MethodType.methodType(void.class, String.class);
            MAKE = lookup.findConstructor(Mode.class, type);
        } catch (Exception e) {
            throw new InternalError(e);
        }
    }

    static final Mode SYNC = make("SYNC");

    static Mode make(String name) {
        try {
            return (Mode) MAKE.invoke(name);
        } catch (Throwable e) {
            throw new InternalError(e);
        }
    }

    public static void main(String[] args) {
        System.out.println(Mode.PLAIN);
        System.out.println(SYNC);
        System.out.println(make("other"));
        System.out.println(SYNC == make("SYNC"));
    }
}
